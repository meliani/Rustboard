# Contributing to Rustboard

Thank you for your interest! Rustboard is intentionally small and approachable. This guide covers everything you need to contribute code, plugins, or documentation.

---

## Table of Contents

1. [Project structure](#project-structure)
2. [Development setup](#development-setup)
3. [Running locally](#running-locally)
4. [Writing a plugin](#writing-a-plugin)
5. [Release process](#release-process)
6. [Submitting changes](#submitting-changes)

---

## Project structure

```
Rustboard/
├── core/                   # HTTP server (Axum) + SSE/WebSocket + SSH runner
│   ├── src/
│   │   ├── main.rs         # Route wiring and app entry point
│   │   ├── config.rs       # YAML service/preference loading
│   │   ├── health.rs       # Background health-check worker
│   │   ├── discover.rs     # Docker auto-discovery over SSH
│   │   ├── plugin.rs       # Plugin discovery & exec
│   │   ├── service.rs      # Service model
│   │   └── ssh.rs          # SSH command runner
│   └── web/
│       └── index.html      # Single-file web UI (vanilla JS + HTMX)
├── cli/                    # Command-line interface (calls core HTTP API)
├── plugins/                # Plugin trait definition (DashboardPlugin)
├── plugin-openai-tester/   # Bundled example plugin (stdin→stdout JSON)
├── config/
│   ├── services.yaml       # Your local service definitions
│   └── services.example.yaml
├── scripts/
│   ├── build-local.ps1 / .sh          # Build native binaries
│   ├── release.ps1 / .sh              # Tag & trigger a release
│   └── install-plugin-openai-tester.* # Install the bundled plugin
├── install.sh              # End-user installer (Linux/macOS)
├── install.ps1             # End-user installer (Windows)
└── .github/workflows/
    └── release.yml         # CI: auto-tag + cross-compile + publish release
```

---

## Development setup

**Prerequisites**
- [Rust](https://rustup.rs/) stable toolchain
- `cargo` in PATH
- SSH access to any remote hosts you want to monitor (optional for local dev)

**Clone and build**
```bash
git clone https://github.com/meliani/Rustboard.git
cd Rustboard
cargo build --workspace
```

**On Windows** — the project targets Linux environments. Use WSL or the provided helper:
```powershell
.\scripts\ensure-wsl.ps1 cargo build --workspace
```

---

## Running locally

**Start the core server** (serves web UI on `http://localhost:8080`):
```bash
cargo run -p core -- config/services.example.yaml
```

**Use the CLI** (talks to the running core server):
```bash
cargo run -p cli -- list
cargo run -p cli -- start <service-id>
```

**Run tests:**
```bash
cargo test --workspace
```

**Install the bundled plugin** for local testing:
```bash
./scripts/install-plugin-openai-tester.sh          # debug build
./scripts/install-plugin-openai-tester.sh --release
```

Set `PLUGIN_DIR` to point the server at a custom plugin directory:
```bash
PLUGIN_DIR=./plugins/bin cargo run -p core -- config/services.example.yaml
```

---

## Writing a plugin

Plugins are **standalone executables** dropped into the `plugins/` directory. The protocol is simple:

- **stdin** → JSON input (arbitrary, defined by your plugin)
- **stdout** → JSON output: `{ "ok": true, ... }` or `{ "ok": false, "error": "..." }`

Invoke via the API:
```
POST /plugins/exec
{ "name": "my-plugin", "input": { ... } }
```

**Quickstart** — copy the bundled plugin as a template:
```bash
cp -r plugin-openai-tester plugin-my-plugin
# edit plugin-my-plugin/src/main.rs and Cargo.toml
```

Add the new crate to `Cargo.toml`:
```toml
[workspace]
members = [
    ...
    "plugin-my-plugin",
]
```

Alternatively, plugins can be written in **any language** — the only contract is stdin/stdout JSON.

---

## Release process

### Automatic (every merge to `main`)

Every push or merged PR to `main` automatically:
1. Bumps the **patch** version (e.g. `v0.1.3 → v0.1.4`) and pushes a git tag
2. Cross-compiles for Linux x86_64, Windows x86_64, macOS Intel, macOS Apple Silicon
3. Publishes a GitHub Release with all binaries attached

No manual steps required for regular patches.

### Manual version bump (minor / major)

When you need a minor or major release:

```powershell
# Windows
.\scripts\release.ps1 minor   # 0.1.x → 0.2.0
.\scripts\release.ps1 major   # x.y.z → 1.0.0
.\scripts\release.ps1 1.5.0  # explicit
```
```bash
# Linux / macOS
./scripts/release.sh minor
./scripts/release.sh 1.5.0
```

The script validates you're on `main` with a clean tree, confirms the new tag, pushes it, and the CI workflow takes over from there.

---

## Submitting changes

1. Fork the repo and create a branch from `main`.
2. Make your changes and ensure `cargo test --workspace` passes.
3. Open a Pull Request — keep the description concise and focused.
4. Once merged to `main`, a release is published automatically.

For large features or breaking changes, open an issue first to discuss the approach.
