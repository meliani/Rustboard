# Rustboard — API Reference

> **Base URL:** `http://localhost:8080` (default; override with `--server` flag in the CLI or by pointing your client at the host/port where `core` is running)

All request and response bodies are JSON. All responses include a top-level `"ok"` boolean. Errors return HTTP 200 with `{"ok": false, "error": "..."}` (HTTP 4xx/5xx are only returned for routing or body-parsing failures).

---

## Table of Contents

- [Health](#health)
- [Services](#services)
  - [List services](#get-services)
  - [Service command](#post-servicescmd)
  - [Fetch logs](#post-serviceslogs)
  - [Quick command](#post-servicesquick)
  - [Async exec (background job)](#post-servicesexec)
  - [List jobs](#get-servicesjobs)
  - [Get job](#get-servicesjobsid)
- [Discovery](#discovery)
  - [Discover containers](#post-discover)
  - [Forget host](#post-discoverforget)
- [Configuration](#configuration)
  - [Reload config](#post-configreload)
  - [Get preferences](#get-preferences)
- [Topology](#get-topology)
- [Plugins](#plugins)
  - [List plugins](#get-plugins)
  - [Execute plugin](#post-pluginsexec)
- [Machine Info](#machine-info)
  - [Disk usage](#get-machinesdisk)
  - [Docker system df](#get-machinesdocker)
- [Real-time: SSE](#get-events)
- [Real-time: WebSocket](#get-ws)

---

## Health

### `GET /health`

Liveness check. Returns the string `ok` (not JSON).

**Response**
```
ok
```

---

## Services

### `GET /services`

Returns the complete list of all services known to the server (both YAML-defined and auto-discovered).

**Response**
```json
{
  "services": [
    {
      "id": "my-api",
      "name": "My API Server",
      "host": "10.0.2.10",
      "port": 3000,
      "ssh_user": "ubuntu",
      "status": "running",
      "tags": ["api", "production"],
      "stacks": ["my-project"],
      "dependencies": [],
      "discovered": false,
      "start_cmd": "systemctl start my-api",
      "stop_cmd": "systemctl stop my-api",
      "restart_cmd": "systemctl restart my-api",
      "log_path": "/var/log/my-api.log",
      "log_cmd": null,
      "health_cmd": null,
      "health_path": "/health",
      "container_name": null,
      "container_id": null,
      "image": null,
      "predicted_app_path": null,
      "quick_commands": [
        { "name": "shell", "cmd": "bash -l", "description": "Interactive shell", "in_container": false }
      ]
    }
  ]
}
```

See [docs/CONFIGURATION.md](CONFIGURATION.md) for the full field reference.

---

### `POST /services/cmd`

Execute a lifecycle command for a service via SSH. The status is optimistically updated to the action name while the command runs, then set to the final state.

**Request body**
```json
{ "id": "my-api", "cmd": "start" }
```

| Field | Type | Values |
|---|---|---|
| `id` | string | Service ID |
| `cmd` | string | `"start"` · `"stop"` · `"restart"` · any custom string (passed directly to SSH) |

**Response — success**
```json
{ "ok": true, "output": "..." }
```

**Response — failure**
```json
{ "ok": false, "error": "ssh failed: Connection refused" }
```

> Commands broadcast a `service_update` event to all connected SSE/WebSocket clients both when they start (optimistic) and when they complete.

---

### `POST /services/logs`

Fetch recent log lines for a service. The server uses `log_cmd` if configured, otherwise falls back to `log_path` via `tail -n <lines>`.

**Request body**
```json
{ "id": "my-api", "lines": 200 }
```

| Field | Type | Default |
|---|---|---|
| `id` | string | — |
| `lines` | integer | `200` |

**Response**
```json
{ "ok": true, "logs": "2024-01-01 10:00:00 INFO server started\n..." }
```

---

### `POST /services/quick`

Execute a named quick command for a service. If the quick command has `in_container: true` and the service has a `container_name`, the command is wrapped in `docker exec -it <container_name> <cmd>`.

**Request body**
```json
{ "id": "my-api", "quick": "shell" }
```

**Response**
```json
{ "ok": true, "output": "..." }
```

---

### `POST /services/exec`

Submit a long-running command as a **background job**. The endpoint returns immediately with a job ID; the command runs asynchronously and its output can be streamed via the job endpoints.

**Request body**
```json
{ "id": "my-api", "cmd": "npm run migrate" }
```

**Response**
```json
{ "ok": true, "job_id": "018f3b2a4c1000abc" }
```

---

### `GET /services/jobs`

List all background jobs (running and completed).

**Response**
```json
{
  "ok": true,
  "jobs": [
    {
      "id": "018f3b2a4c1000abc",
      "service_id": "my-api",
      "cmd": "npm run migrate",
      "state": "done",
      "output": ["Migrating...", "Done."],
      "exit_code": 0,
      "started_at": 1715000000000,
      "finished_at": 1715000005200
    }
  ]
}
```

**Job states:** `"running"` · `"done"` · `"failed"`

---

### `GET /services/jobs/:id`

Get a single job by ID.

**Response**
```json
{
  "ok": true,
  "job": { ... }
}
```

Returns `{"ok": false, "error": "job not found"}` if the ID doesn't exist.

---

## Discovery

### `POST /discover`

Trigger Docker container discovery on a remote host via SSH. Discovered containers are merged into the service list and persisted to `config/discovered/<host>.yaml`.

Previously-discovered services for the same host are refreshed (container ID, commands, status). YAML-defined services are never overwritten.

**Request body**
```json
{ "host": "10.0.2.10", "ssh_user": "ubuntu" }
```

| Field | Type | Required |
|---|---|---|
| `host` | string | ✅ |
| `ssh_user` | string | ❌ (uses current user if omitted) |

**Response**
```json
{
  "ok": true,
  "discovered": 5,
  "new": 3,
  "updated": 2
}
```

A `full_state` broadcast is sent to all connected clients after successful discovery.

---

### `POST /discover/forget`

Remove all discovered services for a host and delete the persisted `config/discovered/<host>.yaml` file.

**Request body**
```json
{ "host": "10.0.2.10" }
```

**Response**
```json
{ "ok": true }
```

---

## Configuration

### `POST /config/reload`

Hot-reload the service configuration from disk. Reads the config path that was passed to the server at startup, merges `config/discovered/*.yaml`, and reloads `config/preferences.yaml`.

Broadcasts a `full_state` event to all connected clients.

**Request body:** none (empty body or `{}`)

**Response**
```json
{ "ok": true }
```

---

### `GET /preferences`

Returns current UI preferences.

**Response**
```json
{
  "show_tooltips": true,
  "theme": "dark"
}
```

---

## Topology

### `GET /topology`

Returns a simple dependency graph suitable for network topology visualisation.

**Response**
```json
{
  "nodes": ["my-api", "database", "cache"],
  "edges": [
    { "from": "my-api", "to": "database" },
    { "from": "my-api", "to": "cache" }
  ]
}
```

Edges are derived from the `dependencies` array in each service's config.

---

## Plugins

### `GET /plugins`

List all installed plugins (stems of `.wasm` files in the plugin directory).

**Response**
```json
{ "ok": true, "plugins": ["plugin-openai-tester", "my-plugin"] }
```

---

### `POST /plugins/exec`

Execute a plugin by name.

**Request body**
```json
{
  "name": "plugin-openai-tester",
  "input": { "api_key": "sk-...", "base_url": "https://api.openai.com/v1" }
}
```

| Field | Type | Notes |
|---|---|---|
| `name` | string | File stem (no `.wasm`). Path separators and `..` are rejected with a 200 error. |
| `input` | any JSON value | Serialised to a string and passed to the plugin's `execute` function. Omit or send `null` for plugins that take no input. |

**Response — success**
```json
{ "ok": true, "output": "{\"ok\":true,\"valid\":true,\"models\":[\"gpt-4o\"]}" }
```

**Response — plugin not found**
```json
{ "ok": false, "error": "plugin not found: plugins/bin/my-plugin.wasm" }
```

**Response — execution error**
```json
{ "ok": false, "error": "executing plugin function: ..." }
```

> The `output` field is the **raw string** returned by the plugin's `execute` function — typically JSON itself. Parse it client-side to access plugin-specific fields.

---

## Machine Info

These endpoints query a **remote host** via SSH and return system-level metrics. No `SharedState` is required — they can target any SSH-accessible host.

### `GET /machines/disk`

Returns disk usage for `/` on the remote host (via `df -k /`).

**Query parameters**

| Parameter | Required | Example |
|---|---|---|
| `host` | ✅ | `10.0.2.10` |
| `ssh_user` | ❌ | `ubuntu` |

**Example**
```
GET /machines/disk?host=10.0.2.10&ssh_user=ubuntu
```

**Response**
```json
{ "ok": true, "total_kb": 102400000, "used_kb": 40960000, "avail_kb": 61440000 }
```

---

### `GET /machines/docker`

Returns Docker system disk usage (`docker system df`) on the remote host.

**Query parameters:** same as `/machines/disk`

**Response**
```json
{
  "ok": true,
  "items": [
    { "Type": "Images", "TotalCount": "12", "Active": "8", "Size": "4.2GB", "Reclaimable": "1.1GB" },
    { "Type": "Containers", "TotalCount": "6", "Active": "5", "Size": "128MB", "Reclaimable": "0B" }
  ]
}
```

---

## `GET /events`

**Server-Sent Events (SSE)** — real-time push stream.

Connect with the browser's `EventSource` API or any SSE client.

```js
const es = new EventSource('/events');
es.onmessage = (e) => {
  const payload = JSON.parse(e.data);
  // payload.type === "full_state" | "service_update"
};
```

**On connect:** the server immediately sends a `full_state` snapshot.

**Event payload types**

#### `full_state`

Sent on: initial connect · `POST /config/reload` · discovery operations

```json
{
  "type": "full_state",
  "services": [ { ...Service... }, ... ]
}
```

#### `service_update`

Sent on: health status change · SSH command started · SSH command completed

```json
{
  "type": "service_update",
  "service": { ...Service... }
}
```

The broadcast channel has capacity 64. Slow consumers that fall 64 messages behind will receive a `BroadcastStream` lag error and be dropped.

---

## `GET /ws`

**WebSocket** — bidirectional real-time channel.

Receives the same broadcast messages as SSE, and also accepts inbound command messages.

**Client → server message**
```json
{
  "type": "cmd",
  "id":   "my-api",
  "cmd":  "restart"
}
```

The server spawns `perform_service_cmd` in a background task and the result is broadcast to all clients (including the sender) via the broadcast channel.

**Server → client messages:** identical to SSE event payloads (JSON strings).

---

## Error reference

| Error message | Meaning |
|---|---|
| `"service not found"` | No service with the given `id` exists |
| `"no command configured"` | `start_cmd` / `stop_cmd` / `restart_cmd` is null and no custom command was given |
| `"no logs configured"` | Neither `log_cmd` nor `log_path` is set for the service |
| `"quick command not found"` | No `quick_commands` entry matches the given `quick` name |
| `"plugin not found: ..."` | No `.wasm` file with the given name exists in the plugin directory |
| `"invalid plugin name"` | Plugin name contains `/`, `\`, or `..` (path traversal attempt) |
| `"ssh failed: ..."` | SSH process returned non-zero exit code; stderr included |
| `"failed to spawn ssh"` | System `ssh` binary not found or not executable |


Endpoints:

- `GET /health` — simple health check (returns `ok`).
- `GET /services` — list services (JSON: `{ services: [ ... ] }`).
- `POST /services/cmd` — execute a command for a service.
  - Body (JSON): `{ "id": "service-id", "cmd": "start|stop|restart|..." }`
  - Response (JSON): `{ "ok": true, "output": "..." }` or `{ "ok": false, "error": "..." }`
- `POST /services/logs` — fetch recent logs for a service.
  - Body (JSON): `{ "id": "service-id", "lines": 200 }
  - Response (JSON): `{ "ok": true, "logs": "..." }`
- `POST /config/reload` — reload `config/services.example.yaml` (server reads config path from process args when starting).
  - Response: `{ "ok": true }` or `{ "ok": false, "error": "..." }`
- `GET /topology` — returns a simple graph JSON `{ nodes: [...], edges: [{from, to}, ...] }` based on `dependencies` in the service config.

Notes:
- The server reads `config/services.example.yaml` by default. Pass a different config path as the first CLI argument to the `core` binary.
- Commands are executed via SSH using the system `ssh` binary (so run the server in an environment with `ssh` available, e.g., WSL/Linux).

---

## Plugin API

Plugins are **WebAssembly modules** loaded at runtime via [Extism](https://extism.org). They are sandboxed — they cannot access the filesystem or spawn processes unless the host explicitly grants permission.

### Plugin directory

The server scans for `*.wasm` files in the plugin directory at startup:

| Context | Default path |
|---|---|
| Installed binary | `plugins/bin/` next to the executable |
| `cargo run` (dev) | `plugins/bin/` relative to workspace root |
| Override | Set `PLUGIN_DIR` environment variable |

### `GET /plugins`

List all installed plugins (stems of `.wasm` files in the plugin directory).

**Response**
```json
{ "ok": true, "plugins": ["plugin-openai-tester", "my-plugin"] }
```

### `POST /plugins/exec`

Execute a plugin by name.

**Request body**
```json
{
  "name": "plugin-openai-tester",
  "input": { ... }
}
```

- `name` — plugin stem (without `.wasm`). Path separators and `..` are rejected.
- `input` — any JSON value; serialised to a string and passed to the plugin's `execute` function. Omit or send `null` for plugins that take no input.

**Response**
```json
{ "ok": true, "output": "<raw string returned by plugin>" }
```
or on failure:
```json
{ "ok": false, "error": "plugin not found" }
```

The `output` field is the raw string returned by the plugin — typically JSON itself (see each plugin's own schema).

---

## Bundled plugins

### `plugin-openai-tester`

Tests whether an OpenAI-compatible API key is valid by calling `/models` on the configured base URL. Works with any OpenAI-compatible endpoint (Azure OpenAI, Ollama, Groq, Together AI, etc.).

**Input**
```json
{
  "api_key": "sk-...",
  "base_url": "https://api.openai.com/v1"
}
```
`base_url` is optional and defaults to `https://api.openai.com/v1`.

**Output — key valid**
```json
{ "ok": true, "valid": true, "models": ["gpt-4o", "gpt-4o-mini", "..."] }
```

**Output — key invalid / unauthorised**
```json
{ "ok": true, "valid": false, "error": "Incorrect API key provided" }
```

**Output — network or input error**
```json
{ "ok": false, "error": "network error: ..." }
```
