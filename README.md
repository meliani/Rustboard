<div align="center">

<img src="https://img.shields.io/badge/built_with-Rust-orange?logo=rust&logoColor=white" alt="Built with Rust">
<a href="https://github.com/meliani/Rustboard/actions/workflows/release.yml"><img src="https://github.com/meliani/Rustboard/actions/workflows/release.yml/badge.svg?branch=main" alt="Release"></a>
<img src="https://img.shields.io/badge/license-MIT-blue" alt="MIT License">
<img src="https://img.shields.io/badge/plugins-WebAssembly-purple?logo=webassembly&logoColor=white" alt="WASM Plugins">
<img src="https://img.shields.io/badge/protocol-SSH%20%7C%20SSE%20%7C%20WebSocket-brightgreen" alt="Protocols">

# 🦀 Rustboard

**A lightweight, zero-dependency microservices dashboard built in Rust.**

Real-time health monitoring · SSH-powered service control · WebAssembly plugin system · Docker auto-discovery

[Quick Start](#-quick-start) · [Architecture](docs/ARCHITECTURE.md) · [Plugin Guide](docs/PLUGINS.md) · [API Reference](docs/API.md) · [Configuration](docs/CONFIGURATION.md) · [Contributing](docs/CONTRIBUTING.md)

</div>

---

## What is Rustboard?

Rustboard is an **open-source developer tool** for teams running microservices on private infrastructure — bare-metal boxes, home labs, VPS fleets, or Docker hosts you access over SSH.

It gives you a **single pane of glass**: a dark-themed web dashboard, a scriptable CLI, and a sandboxed plugin runtime — all in a single ~10 MB binary with no runtime dependencies beyond the system `ssh` client.

```
┌─────────────────────────────────────────────────────────────┐
│  Browser  →  WebSocket / SSE  →  core server (Axum/Tokio)  │
│                                        │                    │
│  CLI  →────────────── REST  ──────────►│                    │
│                                        │                    │
│  Plugins (.wasm)  ←── Extism  ─────────┘                    │
│                            │                                │
│                     SSH  → Remote hosts                     │
└─────────────────────────────────────────────────────────────┘
```

## Features

## ✨ Features

| Feature | Details |
|---|---|
| **Real-time dashboard** | Live service status via Server-Sent Events (SSE) + WebSocket |
| **SSH-powered control** | Start · stop · restart services on any SSH-reachable host |
| **Log tailing** | Fetch recent logs from remote files or custom log commands |
| **Quick commands** | Per-service shell shortcuts (migrations, interactive shells, etc.) |
| **Docker auto-discovery** | Detect running containers on a host and auto-generate service entries |
| **Background health checks** | Tri-strategy: SSH health cmd → HTTP probe → TCP fallback (every 10 s) |
| **Network topology** | Dependency graph view built from your service YAML |
| **WebAssembly plugins** | Sandboxed extensions loaded at runtime — write in Rust, Go, Python, JS, or any WASI language |
| **Zero-framework web UI** | Single HTML file — vanilla JS + HTMX, dark theme, no build step |
| **Scriptable CLI** | Full REST client for automation and CI pipelines |

---

## 🚀 Quick Start

### Option A — Pre-built binary (recommended)

**Linux / macOS**
```bash
curl -fsSL https://raw.githubusercontent.com/meliani/Rustboard/main/install.sh | bash
```

**Windows (PowerShell)**
```powershell
irm https://raw.githubusercontent.com/meliani/Rustboard/main/install.ps1 | iex
```

The installer detects your platform, downloads the latest release binaries from [GitHub Releases](https://github.com/meliani/Rustboard/releases), and adds them to your `PATH`.

Then start the server:
```bash
rustboard-core config/services.yaml
# Open http://localhost:8080
```

### Option B — Build from source

**Prerequisites:** Rust ≥ 1.75 stable, `wasm32-wasip1` target, system `ssh` binary.

```bash
git clone https://github.com/meliani/Rustboard.git
cd Rustboard

# Build core server and CLI
cargo build --release

# Copy to working directory
./scripts/build-local.sh          # Linux/macOS
.\scripts\build-local.ps1         # Windows (PowerShell)

# Start the server
./rustboard-core config/services.example.yaml

# In another terminal, use the CLI
./rustboard-cli list
```

> **Windows users:** This project targets Linux environments. Use WSL or the helper:
> ```powershell
> .\scripts\ensure-wsl.ps1 cargo run -p core -- config/services.example.yaml
> ```

### Configure your services

Edit `config/services.yaml` (copy `config/services.example.yaml` as your starting point):

```yaml
- id: "my-api"
  name: "My API Server"
  host: "10.0.2.10"
  port: 3000
  ssh_user: "ubuntu"
  start_cmd: "systemctl start my-api"
  stop_cmd: "systemctl stop my-api"
  restart_cmd: "systemctl restart my-api"
  health_path: "/health"
  log_path: "/var/log/my-api.log"
  tags: ["api", "production"]
  stacks: ["my-project"]
  quick_commands:
    - name: "shell"
      cmd: "bash -l"
      description: "Interactive shell"
    - name: "migrate"
      cmd: "cd /app && ./migrate.sh"
      description: "Run database migrations"
```

Open `http://localhost:8080` and your services appear immediately. No refresh needed — all updates are pushed over WebSocket/SSE.

---

## 🏗️ Architecture Overview

Rustboard is a **Cargo workspace** of four crates plus one WASM plugin:

```
Rustboard/
├── core/          # HTTP server: Axum + Tokio + SSE + WebSocket + SSH runner
├── cli/           # REST CLI client (clap)
├── web/           # Placeholder crate (UI lives in core/web/index.html)
├── plugins/       # Shared crate (plugin trait stub)
└── plugin-openai-tester/  # Example WASM plugin (Extism PDK)
```

```
                        ┌────────────────────────────────────────┐
                        │              core (Axum)               │
   Browser / CLI        │                                        │
        │               │  ┌──────────┐   ┌──────────────────┐  │
        │ REST/JSON      │  │ AppState │   │ Background Tasks │  │
        │◄──────────────►│  │ services │   │  health check    │  │
        │                │  │ prefs    │   │  auto-rediscovery│  │
        │ SSE (events)   │  │ jobs     │   └──────────────────┘  │
        │◄──────────────  │  └──────────┘                        │
        │                │        │                              │
        │ WebSocket      │        ▼                              │
        │◄──────────────►│  Broadcast channel (tokio::broadcast) │
                         │        │                              │
                         │        ▼                              │
                         │   SSH runner ──► Remote hosts         │
                         │        │                              │
                         │   Extism host ──► *.wasm plugins      │
                         └────────────────────────────────────────┘
```

For a detailed breakdown of every subsystem, see [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

---

## 🔌 Plugin System

Plugins are **WebAssembly modules** (`*.wasm`) loaded at runtime via [Extism](https://extism.org). They are fully sandboxed — no filesystem access, no process spawning.

The contract is simple:

```
POST /plugins/exec  { "name": "my-plugin", "input": { ... } }
                                    │
                            execute(json_string) → json_string
```

Write a plugin in **any WASI-compatible language** (Rust, Go, Python, JavaScript, C, Zig, …):

```rust
// Rust plugin skeleton
use extism_pdk::*;

#[plugin_fn]
pub fn execute(raw: String) -> FnResult<String> {
    // parse raw as JSON, do work, return JSON
    Ok(r#"{"ok":true,"result":"hello"}"#.to_string())
}
```

### Bundled plugin: `plugin-openai-tester`

Tests any OpenAI-compatible API key (OpenAI, Azure, Ollama, Groq, Together AI, etc.):

```bash
# Build and install
./scripts/install-plugin-openai-tester.sh --release

# Invoke
curl -s -X POST http://localhost:8080/plugins/exec \
  -H 'Content-Type: application/json' \
  -d '{"name":"plugin-openai-tester","input":{"api_key":"sk-..."}}'

# → {"ok":true,"output":"{\"ok\":true,\"valid\":true,\"models\":[\"gpt-4o\",...]}"}
```

**[Full Plugin Developer Guide →](docs/PLUGINS.md)**

---

## 🖥️ CLI Reference

```bash
rustboard-cli [--server http://host:8080] <command>

Commands:
  list                          List all services and their status
  status <id>                   Show full JSON for a service
  start <id>                    Start a service
  stop <id>                     Stop a service
  restart <id>                  Restart a service
  logs <id> [-l <lines>]        Fetch logs (default: 200 lines)
  quick-list <id>               List quick commands for a service
  quick-exec <id> <quick>       Execute a quick command
  config-reload                 Hot-reload config on the running server
```

---

## 📡 API Quick Reference

The core server exposes a REST + SSE + WebSocket API on port `8080`:

| Method | Path | Description |
|---|---|---|
| `GET` | `/health` | Liveness check |
| `GET` | `/services` | List all services |
| `POST` | `/services/cmd` | Start / stop / restart a service |
| `POST` | `/services/logs` | Fetch service logs |
| `POST` | `/services/quick` | Execute a quick command |
| `POST` | `/services/exec` | Async background job execution |
| `GET` | `/services/jobs` | List background jobs |
| `GET` | `/services/jobs/:id` | Get job status and output |
| `POST` | `/discover` | Discover Docker containers on a host |
| `POST` | `/discover/forget` | Remove a host's discovered services |
| `GET` | `/topology` | Service dependency graph |
| `GET` | `/preferences` | Read UI preferences |
| `GET` | `/events` | SSE stream (real-time updates) |
| `GET` | `/ws` | WebSocket (bidirectional commands + updates) |
| `GET` | `/plugins` | List installed plugins |
| `POST` | `/plugins/exec` | Execute a plugin |

**[Full API Reference →](docs/API.md)**

---

## 📁 Repository Structure

```
Rustboard/
├── core/
│   ├── src/
│   │   ├── main.rs          # Route wiring, AppState, background workers
│   │   ├── config.rs        # YAML loading, discovered-service persistence
│   │   ├── health.rs        # Health check logic (SSH → HTTP → TCP)
│   │   ├── discover.rs      # Docker auto-discovery over SSH
│   │   ├── plugin.rs        # WASM plugin loader (Extism)
│   │   ├── service.rs       # Service and QuickCommand structs
│   │   └── ssh.rs           # SSH command runner (blocking + streaming)
│   └── web/
│       └── index.html       # Single-file web UI (vanilla JS + HTMX)
├── cli/
│   └── src/main.rs          # CLI entry point (clap)
├── plugins/
│   └── src/lib.rs           # Shared plugin trait stub
├── plugin-openai-tester/
│   └── src/lib.rs           # Reference WASM plugin
├── config/
│   ├── services.yaml        # Your service definitions (gitignored)
│   ├── services.example.yaml
│   └── preferences.yaml
├── scripts/
│   ├── build-local.sh / .ps1
│   ├── release.sh / .ps1
│   └── install-plugin-openai-tester.sh / .ps1
├── docs/
│   ├── ARCHITECTURE.md      # Deep architecture docs
│   ├── PLUGINS.md           # Plugin developer guide
│   ├── API.md               # Full API reference
│   ├── CONFIGURATION.md     # Config file reference
│   ├── CONTRIBUTING.md      # Development guide
│   └── SECURITY.md          # Security model
├── install.sh               # End-user installer (Linux/macOS)
└── install.ps1              # End-user installer (Windows)
```

---

## 🔁 Releases

Every push to `main` automatically:
1. Bumps the patch version (`v0.1.3 → v0.1.4`)
2. Cross-compiles for **Linux x86_64**, **Windows x86_64**, **macOS Intel**, **macOS Apple Silicon**
3. Publishes a [GitHub Release](https://github.com/meliani/Rustboard/releases) with all binaries attached

For a manual minor/major release:
```bash
./scripts/release.sh minor   # bumps minor version
./scripts/release.sh 2.0.0   # sets explicit version
```

---

## 🤝 Contributing

Contributions are very welcome! Whether it's a bug fix, a new plugin, or documentation improvements — see [docs/CONTRIBUTING.md](docs/CONTRIBUTING.md) for:

- Project structure and local dev setup
- Running tests
- Writing and installing plugins
- The release process in detail
- How to submit a PR

---

## 📚 Documentation Index

| Document | Contents |
|---|---|
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | Crate map, data flows, concurrency model, SSE/WS protocol internals |
| [docs/PLUGINS.md](docs/PLUGINS.md) | Complete plugin developer guide (Rust, Go, JS, Python) |
| [docs/API.md](docs/API.md) | Full REST + SSE + WebSocket API reference |
| [docs/CONFIGURATION.md](docs/CONFIGURATION.md) | `services.yaml` field reference, environment variables |
| [docs/CONTRIBUTING.md](docs/CONTRIBUTING.md) | Dev setup, testing, release workflow, PR checklist |
| [docs/SECURITY.md](docs/SECURITY.md) | SSH security model, WASM sandboxing, threat model |

---

## 📄 License

MIT — see [LICENSE](LICENSE) for details.

---

<div align="center">
Made with 🦀 Rust · If Rustboard helps your workflow, please ⭐ the repo!
</div>

