# Rustboard — Architecture

This document is the definitive technical reference for Rustboard's internals. It covers the crate structure, runtime data model, every protocol layer, background workers, and the full request lifecycle from browser click to SSH command.

---

## Table of Contents

1. [Crate Map](#1-crate-map)
2. [Core Crate — Module Breakdown](#2-core-crate--module-breakdown)
3. [Application State](#3-application-state)
4. [Request Lifecycle](#4-request-lifecycle)
5. [Real-time Transport Layer](#5-real-time-transport-layer)
   - [Server-Sent Events (SSE)](#server-sent-events-sse)
   - [WebSocket](#websocket)
6. [SSH Execution Engine](#6-ssh-execution-engine)
7. [Health Check System](#7-health-check-system)
8. [Docker Discovery Engine](#8-docker-discovery-engine)
9. [Plugin Runtime (WebAssembly / Extism)](#9-plugin-runtime-webassembly--extism)
10. [Background Job System](#10-background-job-system)
11. [Configuration & Persistence](#11-configuration--persistence)
12. [Web UI Architecture](#12-web-ui-architecture)
13. [CLI Architecture](#13-cli-architecture)
14. [Dependency Tree](#14-dependency-tree)
15. [Data Flow Diagrams](#15-data-flow-diagrams)

---

## 1. Crate Map

Rustboard is organised as a **Cargo workspace** with five crates:

```
Rustboard/                        (workspace root)
├── core/          (bin)          HTTP server — Axum/Tokio, all business logic
├── cli/           (bin)          REST CLI client — clap + reqwest
├── web/           (lib)          Placeholder crate; UI lives in core/web/index.html
├── plugins/       (lib)          Shared stub (reserved for future plugin trait)
└── plugin-openai-tester/ (cdylib) Reference WASM plugin — compiled to wasm32-wasip1
```

### Build targets

| Crate | Default target | Notes |
|---|---|---|
| `core` | native | Requires Linux `ssh` binary at runtime |
| `cli` | native | No OS dependencies |
| `plugins` | native | Library only — currently a stub |
| `plugin-openai-tester` | `wasm32-wasip1` | Must be built with `--target wasm32-wasip1`; excluded from `default-members` |

---

## 2. Core Crate — Module Breakdown

```
core/src/
├── main.rs      — Router wiring, AppState construction, background task spawning
├── config.rs    — YAML loading, discovered-service persistence
├── service.rs   — Service and QuickCommand data model
├── health.rs    — Health check logic (three-strategy cascade)
├── discover.rs  — Docker auto-discovery over SSH
├── plugin.rs    — WASM plugin enumeration and execution
└── ssh.rs       — SSH command runner (blocking + streaming variants)
```

### `main.rs`

The entry point performs four responsibilities at startup:

1. **Parse CLI args** — optional config path (default: `config/services.example.yaml`)
2. **Build `AppState`** — load YAML config, initialise broadcast channel, create `Arc<AppState>`
3. **Spawn background workers** — health-check poller and auto-rediscovery poller
4. **Mount Axum router** — bind every HTTP/WS/SSE route, serve embedded UI HTML

All route handlers are written as `move` closures that capture an `Arc<AppState>` clone so the compiler can satisfy Axum's `Handler` trait bounds without lifetime issues.

---

## 3. Application State

```rust
struct AppState {
    services:    RwLock<Vec<Service>>,
    preferences: RwLock<Preferences>,
    broadcaster: broadcast::Sender<String>,
    jobs:        RwLock<HashMap<String, Job>>,
}

type SharedState = Arc<AppState>;
```

| Field | Type | Purpose |
|---|---|---|
| `services` | `RwLock<Vec<Service>>` | Master list of all services (YAML + discovered). Read-heavy; write only on config reload, discovery, or health-status change. |
| `preferences` | `RwLock<Preferences>` | UI preferences (`theme`, `show_tooltips`). Reloaded via `POST /config/reload`. |
| `broadcaster` | `broadcast::Sender<String>` | Tokio broadcast channel (capacity 64). Every event is serialised to a JSON string and sent here; all SSE/WS clients subscribe. |
| `jobs` | `RwLock<HashMap<String, Job>>` | Background async job map. Keyed by opaque hex ID. |

### Locking discipline

The project follows a strict rule: **never hold a write lock across an `await` point** (specifically across SSH calls). Service commands follow the pattern:

1. Acquire write lock → optimistically update status → clone needed fields → release lock
2. `ssh::run_command(...)` — executes with no lock held
3. Acquire write lock again → write final status → release

This prevents lock contention from turning `O(services)` concurrent commands into a serial bottleneck.

---

## 4. Request Lifecycle

### Service command (`POST /services/cmd`)

```
Client
  │  POST /services/cmd  {"id":"api","cmd":"restart"}
  │
  ▼
Axum router (cmd_route closure)
  │
  ├─ parse JSON body → CmdBody { id, cmd }
  │
  └─ perform_service_cmd(state, id, cmd)
        │
        ├─ LOCK services (write)
        │     find service by id
        │     resolve command string (start_cmd / stop_cmd / restart_cmd)
        │     set status = action (optimistic)
        │     clone (host, ssh_user, cmd)
        │     broadcast { type: "service_update", service: ... }
        │  UNLOCK
        │
        ├─ ssh::run_command(ssh_user, host, cmd)   ← no lock held
        │
        ├─ LOCK services (write)
        │     update status = action (success) OR "error"
        │     broadcast service_update
        │  UNLOCK
        │
        └─ return JSON {"ok": true, "output": "..."}
```

---

## 5. Real-time Transport Layer

Rustboard uses **two complementary transports** for real-time updates. Both consume the same Tokio broadcast channel (`broadcast::Sender<String>`).

### Server-Sent Events (SSE)

**Endpoint:** `GET /events`

SSE is a **server-push, unidirectional** HTTP/1.1 stream. It is the primary transport for the web UI.

**Connection sequence:**
1. Client connects → server immediately sends a `full_state` snapshot of all services
2. Client stays connected; server pushes `Event::default().data(json_string)` on every broadcast message
3. The browser's `EventSource` API handles reconnects automatically

**Event types:**

| `type` field | When emitted | Payload |
|---|---|---|
| `full_state` | On SSE connect, config reload, discovery | `{ "type": "full_state", "services": [...] }` |
| `service_update` | Health check detects status change, SSH command completes | `{ "type": "service_update", "service": { ... } }` |

**Implementation:**

```rust
// Initial snapshot + broadcast stream chained into a single SSE stream
let init_stream = tokio_stream::once(Ok(Event::default().data(init)));
let bs = BroadcastStream::new(rx).filter_map(|res| async move {
    match res { Ok(s) => Some(Ok(Event::default().data(s))), Err(_) => None }
});
Sse::new(init_stream.chain(bs))
```

### WebSocket

**Endpoint:** `GET /ws`

WebSocket is **bidirectional** — clients can send commands to the server (same as the REST API) and receive the same broadcast messages.

**Client → server message format:**

```json
{
  "type": "cmd",
  "id":   "service-id",
  "cmd":  "start | stop | restart | <custom>"
}
```

The WS handler spawns `perform_service_cmd` in a background task for each received command. Updates flow back to the client through the broadcast channel.

**Why both SSE and WebSocket?**

- SSE works through HTTP/1.1 proxies and load balancers without special configuration
- WebSocket enables future interactive features (e.g. terminal streaming)
- The web UI uses SSE for status updates and could use WebSocket for interactive shells

---

## 6. SSH Execution Engine

**File:** `core/src/ssh.rs`

All remote operations go through Tokio's `process::Command` spawning the system `ssh` binary. The SSH options applied globally are:

```
-o BatchMode=yes             # Never prompt for passwords; fail immediately
-o ConnectTimeout=10         # 10-second TCP connection timeout
-o ServerAliveInterval=15    # Send keep-alive probe every 15 seconds
-o ServerAliveCountMax=3     # Drop after 3 unanswered probes (~45 s of silence)
-o StrictHostKeyChecking=accept-new  # Auto-accept new keys; reject changed keys
```

### `run_command` — blocking collect

```rust
pub async fn run_command(ssh_user: Option<&str>, host: &str, cmd: &str) -> Result<String>
```

Runs a command and collects full stdout. Stderr is merged server-side using `2>&1`. Returns `Err` if the SSH process exits with a non-zero code.

### `run_command_streaming` — line-by-line streaming

```rust
pub async fn run_command_streaming(
    ssh_user: Option<&str>,
    host: &str,
    cmd: &str,
    tx: tokio::sync::mpsc::Sender<String>,
) -> Result<i32>
```

Streams each output line through an `mpsc::Sender`. Used by the background job system for long-running commands. Dropping the receiver ends the stream early (the SSH process may continue running on the remote host).

### `tail_file`

Convenience wrapper that runs `tail -n <lines> <path>` on the remote host.

### Authentication

Rustboard relies entirely on the **user's SSH agent or key configuration** (`~/.ssh/config`, `~/.ssh/id_*`, `SSH_AUTH_SOCK`). No credentials are stored in Rustboard. `BatchMode=yes` ensures a missing key causes an immediate error rather than hanging on a password prompt.

---

## 7. Health Check System

**File:** `core/src/health.rs`

A background Tokio task polls every service every **10 seconds** using a three-strategy cascade:

```
check_service(s)
    │
    ├─ 1. health_cmd?  (SSH command)
    │       run remote cmd
    │       output contains "healthy" | "up" | ("running" && !"stopped") → true
    │
    ├─ 2. port or health_path?  (HTTP GET)
    │       GET http://{host}:{port}{health_path}   timeout=2s
    │       2xx → true
    │
    └─ 3. port?  (TCP connect)
            TcpStream::connect({host}:{port})   timeout=1s
            success → true
```

If the computed `is_healthy` result differs from the service's current `status`, the worker acquires a write lock, updates the status, and broadcasts a `service_update` event to all connected clients.

**Status values:** `"running"` · `"stopped"` · `"unknown"` · `"error"` · any custom value from a control command

---

## 8. Docker Discovery Engine

**File:** `core/src/discover.rs`

Discovery runs `docker ps -a --format '{{json .}}'` on a target host over SSH and parses the JSON-per-line output.

### Discovery flow

```
POST /discover  { host: "10.0.2.10", ssh_user: "ubuntu" }
    │
    ▼
ssh::run_command → "docker ps -a --format '{{json .}}'"
    │
    ▼
For each container line:
    parse DockerPsEntry { ID, Names, Image, Ports, Status, Labels, State }
    │
    ├─ id = sanitise(container_name)
    ├─ skip if id already in static (YAML-defined) service set
    ├─ parse labels → stacks, tags
    ├─ infer tags from image name
    ├─ parse first host-mapped port
    ├─ determine status (State field → "running" | "stopped" | ...)
    ├─ build log/start/stop/restart/health commands using container name
    ├─ docker inspect WorkingDir → predicted_app_path
    ├─ docker inspect Mounts → fallback predicted_app_path
    └─ build default quick commands (shell, ls_app)
    │
    ▼
Merge into AppState.services (new entries appended, existing discovered entries refreshed)
    │
    ▼
Persist to config/discovered/<safe_host>.yaml
    │
    ▼
Broadcast full_state
```

### Auto-rediscovery

A separate background task runs every **60 seconds** and re-runs discovery for every host that has at least one `discovered: true` service. This keeps metadata fresh after container redeploys without requiring a manual trigger.

### Label conventions

Rustboard reads Docker labels to populate `stacks` and `tags`:

| Label | Maps to |
|---|---|
| `com.docker.compose.project` | `stacks[]` |
| `rustboard.stack` | `stacks[]` |
| `rustboard.tags` | `tags[]` (comma-separated) |

---

## 9. Plugin Runtime (WebAssembly / Extism)

**File:** `core/src/plugin.rs`

Plugins are `.wasm` files loaded at runtime via the [Extism](https://extism.org) host library.

### Plugin directory resolution

```
1. $PLUGIN_DIR env var  (if set)
2. <executable directory>/bin/  (production: plugins live next to binary)
3. plugins/bin/  (development: cargo run from workspace root)
```

### Execution model

```rust
let wasm   = extism::Wasm::file(&resolved_path);
let manifest = extism::Manifest::new([wasm]).with_allowed_host("*");
let mut plugin = extism::Plugin::new(&manifest, [], true)?;
let result: String = plugin.call("execute", input_str)?;
```

Each plugin execution creates a **fresh plugin instance** — there is no shared state between calls. Execution is run inside `tokio::task::spawn_blocking` to avoid blocking the async runtime.

### Security defaults

| Capability | Status |
|---|---|
| Outbound HTTP (all hosts) | ✅ Granted |
| Filesystem read/write | ❌ Denied |
| Subprocess spawning | ❌ Denied |
| Environment variable access | ❌ Denied |
| Clock / random | ✅ Available via WASI |

See [docs/SECURITY.md](SECURITY.md) for how to restrict network access.

### Plugin contract

```
fn execute(input: String) -> String
```

- **Input:** a JSON string (plugin-defined schema); may be empty
- **Output:** a JSON string; by convention includes `"ok": true|false`
- The host wraps the output in `{"ok": true, "output": "<raw_output>"}` before returning it to the API caller

---

## 10. Background Job System

**File:** `core/src/main.rs` — `POST /services/exec` + `/services/jobs`

The job system enables **long-running asynchronous commands** whose output streams in real time.

```rust
struct Job {
    id:          String,   // opaque hex (timestamp + counter)
    service_id:  String,
    cmd:         String,
    state:       String,   // "running" | "done" | "failed"
    output:      Vec<String>,  // capped at 10 000 lines
    exit_code:   Option<i32>,
    started_at:  u64,   // ms since Unix epoch
    finished_at: Option<u64>,
}
```

### Job lifecycle

1. `POST /services/exec` → creates a `Job`, inserts into `AppState.jobs`, returns `{ "job_id": "..." }`
2. A Tokio task runs `ssh::run_command_streaming`, forwarding each output line via `mpsc`
3. A second task reads the channel and appends lines to `job.output` (capped at 10 000)
4. On completion, `job.state` → `"done"` | `"failed"`, `job.exit_code` and `job.finished_at` are set
5. `GET /services/jobs/:id` polls job status and output (clients may poll or watch SSE)

---

## 11. Configuration & Persistence

### Services YAML

The canonical service list lives in the file passed as the first CLI argument (default: `config/services.example.yaml`). On `POST /config/reload`, the server re-reads this file and merges it with any files in `config/discovered/`.

**Service struct fields** — see [docs/CONFIGURATION.md](CONFIGURATION.md) for the full reference.

### Discovered services

When Docker discovery finds containers on a host, they are persisted to:

```
config/discovered/<safe_host>.yaml
```

The `<safe_host>` is the hostname with all non-alphanumeric, non-hyphen characters replaced with `_`. On startup, `config::load_all_services` merges these files automatically.

**Deduplication rule:** A service from a discovered file is skipped if a service with the same `id` already exists in the main config.

### Preferences YAML

```yaml
# config/preferences.yaml
show_tooltips: true
theme: dark    # "dark" | "light"
```

---

## 12. Web UI Architecture

**File:** `core/web/index.html`

The entire frontend is a **single self-contained HTML file** — no build step, no npm, no framework.

| Layer | Technology |
|---|---|
| Structure | HTML5 + CSS custom properties (CSS variables) |
| Interactivity | Vanilla JavaScript (ES2020) |
| Partial updates | [HTMX](https://htmx.org) 1.9 |
| Icons | [Lucide](https://lucide.dev) (CDN) |
| Fonts | Inter + JetBrains Mono (Google Fonts) |
| Real-time | `EventSource` (SSE) for service updates |

### Layout

```
┌─────────────────────────────────────────────────────────┐
│  Sidebar (260px)  │  Top bar  (sticky, 64px)            │
│  ─ Nav items      │  ─────────────────────────────────  │
│    Dashboard      │  Content area (scrollable)          │
│    Services       │                                     │
│    Topology       │  Service cards / topology view /    │
│    Plugins        │  plugin panel / job log panel       │
│    Settings       │                                     │
└─────────────────────────────────────────────────────────┘
```

The UI connects on load via `EventSource('/events')` and applies `full_state` / `service_update` events to the DOM without full page reloads. HTMX handles form submissions for commands.

---

## 13. CLI Architecture

**File:** `cli/src/main.rs`

The CLI is a thin REST client built with [clap](https://docs.rs/clap) (subcommand model) and [reqwest](https://docs.rs/reqwest) (async HTTP).

All commands call the core server API and print results to stdout. The server URL defaults to `http://127.0.0.1:8080` and can be overridden with `--server`.

```
rustboard-cli --server http://host:port <subcommand>
```

The CLI is intentionally stateless — it carries no local config, all data lives on the server.

---

## 14. Dependency Tree

### `core` key dependencies

| Crate | Version | Purpose |
|---|---|---|
| `axum` | 0.6 | HTTP framework (routing, middleware, SSE, WebSocket) |
| `tokio` | 1 (full) | Async runtime |
| `serde` / `serde_json` / `serde_yaml` | 1 | Serialisation |
| `reqwest` | 0.11 (rustls-tls) | Outbound HTTP for health checks |
| `extism` | 1 | WASM plugin host |
| `tracing` / `tracing-subscriber` | 0.1 / 0.3 | Structured logging |
| `anyhow` | 1 | Error handling |
| `hyper` | 0.14 (full) | Low-level HTTP (used for raw body extraction) |
| `tokio-stream` | 0.1 (sync) | Stream adapters for SSE |
| `futures` / `async-stream` | 0.3 | Async stream utilities |

### `plugin-openai-tester` dependencies

| Crate | Version | Purpose |
|---|---|---|
| `extism-pdk` | 1 | Plugin Development Kit (WASM side) |
| `serde` / `serde_json` | 1 | JSON serialisation |

---

## 15. Data Flow Diagrams

### Service control flow

```
┌────────┐   POST /services/cmd   ┌─────────────────┐
│ Client │──────────────────────►│  core (Axum)     │
└────────┘                        │                  │
                                  │  optimistic      │
    SSE ◄── service_update ◄──── broadcast           │
                                  │                  │
                                  │  ssh::run_command│
                                  │  ───────────────►│──SSH──► Remote host
                                  │  ◄───────────────│◄───── stdout/stderr
                                  │                  │
    SSE ◄── service_update ◄──── broadcast (done)   │
                                  └─────────────────-┘
```

### Plugin execution flow

```
┌────────┐  POST /plugins/exec   ┌─────────────────────────────┐
│ Client │─────────────────────►│  core (Axum)                 │
└────────┘   {name, input}       │                              │
                                  │  validate name (no ..)       │
                                  │                              │
                                  │  plugin::exec_plugin         │
                                  │  ─────────────────────────► │ spawn_blocking
                                  │  extism::Plugin::new(wasm)  │
                                  │  plugin.call("execute", ...) │
                                  │  ◄────────────────────────  │
                                  │                              │
◄───────────────────────────────  │  {"ok":true,"output":"..."}  │
         JSON response            └──────────────────────────────┘
```

### Discovery flow

```
POST /discover { host, ssh_user }
        │
        ▼
  ssh "docker ps -a --format '{{json .}}'"
        │
        ▼
  For each container:
    docker inspect WorkingDir
    docker inspect Mounts
        │
        ▼
  Merge into AppState.services
        │
        ├──► Persist to config/discovered/<host>.yaml
        │
        └──► Broadcast { type: "full_state", services: [...] }
                │
                ▼
        All SSE/WS clients update their UI
```
