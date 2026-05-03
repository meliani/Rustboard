# Contributing to Rustboard

Thank you for your interest! Rustboard is intentionally small and approachable. This guide covers everything you need to contribute code, plugins, or documentation.

Rustboard follows the [Contributor Covenant](https://www.contributor-covenant.org/) Code of Conduct.

---

## Table of Contents

1. [Project structure](#1-project-structure)
2. [Development setup](#2-development-setup)
3. [Running locally](#3-running-locally)
4. [Code style](#4-code-style)
5. [Testing](#5-testing)
6. [Writing a plugin](#6-writing-a-plugin)
7. [Release process](#7-release-process)
8. [Submitting changes](#8-submitting-changes)
9. [Good first issues](#9-good-first-issues)

---

## 1. Project structure

```
Rustboard/
├── core/                   # HTTP server (Axum) + SSE/WebSocket + SSH runner
│   ├── src/
│   │   ├── main.rs         # Route wiring and app entry point
│   │   ├── config.rs       # YAML service/preference loading
│   │   ├── health.rs       # Background health-check worker
│   │   ├── discover.rs     # Docker auto-discovery over SSH
│   │   ├── plugin.rs       # Plugin discovery & exec (Extism)
│   │   ├── service.rs      # Service and QuickCommand structs
│   │   └── ssh.rs          # SSH command runner (blocking + streaming)
│   ├── tests/
│   │   └── config_test.rs
│   └── web/
│       └── index.html      # Single-file web UI (vanilla JS + HTMX)
├── cli/                    # CLI entry point (clap + reqwest)
├── plugins/                # Shared plugin stub crate
├── plugin-openai-tester/   # Reference WASM plugin (Extism PDK)
├── config/
│   ├── services.yaml           # Your local service definitions (gitignored)
│   ├── services.example.yaml
│   └── preferences.yaml
├── docs/
│   ├── ARCHITECTURE.md
│   ├── PLUGINS.md
│   ├── API.md
│   ├── CONFIGURATION.md
│   ├── CONTRIBUTING.md     ← this file
│   └── SECURITY.md
├── scripts/
│   ├── build-local.ps1 / .sh
│   ├── release.ps1 / .sh
│   ├── ensure-wsl.ps1
│   └── install-plugin-openai-tester.*
├── install.sh              # End-user installer (Linux/macOS)
└── install.ps1             # End-user installer (Windows)
```

See [docs/ARCHITECTURE.md](ARCHITECTURE.md) for a deep dive into the codebase.

---

## 2. Development setup

### Prerequisites

| Tool | Minimum version | Install |
|---|---|---|
| Rust | 1.75 (stable) | [rustup.rs](https://rustup.rs/) |
| WASM target | — | `rustup target add wasm32-wasip1` |
| System `ssh` | any | Pre-installed on Linux/macOS; use OpenSSH for Windows/WSL |

### Clone

```bash
git clone https://github.com/meliani/Rustboard.git
cd Rustboard
```

### Build the workspace

```bash
cargo build                        # builds core, cli, web, plugins (native targets)
```

### Build the bundled plugin (optional)

```bash
cargo build -p plugin-openai-tester --target wasm32-wasip1 --release
cp target/wasm32-wasip1/release/plugin_openai_tester.wasm \
   plugins/bin/plugin-openai-tester.wasm
```

Or use the helper script:
```bash
./scripts/install-plugin-openai-tester.sh       # debug build
./scripts/install-plugin-openai-tester.sh --release
```

### Windows

The project targets Linux environments. Use WSL or the helper:

```powershell
# Run any command inside WSL from PowerShell
.\scripts\ensure-wsl.ps1 cargo build --workspace
.\scripts\ensure-wsl.ps1 cargo run -p core -- config/services.example.yaml
```

---

## 3. Running locally

### Start the core server

```bash
# Uses config/services.example.yaml by default
cargo run -p core -- config/services.example.yaml
```

The server starts on `http://localhost:8080`. The web UI is served at the root.

### Override the plugin directory

```bash
PLUGIN_DIR=./plugins/bin cargo run -p core -- config/services.example.yaml
```

### Use the CLI

In a second terminal:

```bash
cargo run -p cli -- list
cargo run -p cli -- status <service-id>
cargo run -p cli -- logs <service-id>
```

### Hot-reload config

Edit `config/services.yaml` while the server is running, then:

```bash
cargo run -p cli -- config-reload
# or
curl -X POST http://localhost:8080/config/reload
```

### Enable debug logging

```bash
RUST_LOG=debug cargo run -p core -- config/services.example.yaml
RUST_LOG=core=debug,warn cargo run -p core -- config/services.example.yaml
```

---

## 4. Code style

- **Formatting:** `cargo fmt` — enforced in CI
- **Lints:** `cargo clippy` — all warnings are addressed before merging
- **Error handling:** Use `anyhow::Result` for fallible functions in `core`; avoid `unwrap()` except in tests and short-lived code paths where a panic is appropriate
- **Async:** Never hold a write lock across an `await` point — clone the needed data, release the lock, then do async work
- **No external HTTP frameworks in the UI:** The web UI stays in `core/web/index.html` as a single file with no build step

```bash
# Check before committing
cargo fmt --check
cargo clippy --workspace -- -D warnings
```

---

## 5. Testing

### Unit and integration tests

```bash
cargo test --workspace
```

Tests currently live in:
- `core/tests/config_test.rs` — YAML config loading
- `core/src/plugin.rs` (inline `#[cfg(test)]`) — plugin discovery and error cases

### Manual integration test checklist

Before submitting a PR that touches core functionality:

- [ ] Server starts cleanly: `cargo run -p core -- config/services.example.yaml`
- [ ] Web UI loads at `http://localhost:8080`
- [ ] `cargo run -p cli -- list` returns a service list
- [ ] Config hot-reload works: edit YAML → `POST /config/reload` → UI updates
- [ ] `GET /health` returns `ok`
- [ ] (If touching discovery) `POST /discover` finds containers on a Docker host

---

## 6. Writing a plugin

See the full guide in [docs/PLUGINS.md](PLUGINS.md).

**Quick steps:**

```bash
# 1. Create crate with cdylib type
cargo new --lib plugin-my-plugin

# 2. Add extism-pdk dependency, set crate-type = ["cdylib"]

# 3. Implement execute() with #[plugin_fn]

# 4. Build
cargo build -p plugin-my-plugin --target wasm32-wasip1 --release

# 5. Install
cp target/wasm32-wasip1/release/plugin_my_plugin.wasm \
   plugins/bin/plugin-my-plugin.wasm

# 6. Test
curl -X POST http://localhost:8080/plugins/exec \
  -d '{"name":"plugin-my-plugin","input":{}}'
```

---

## 7. Release process

### Automatic (every merge to `main`)

Every push to `main` automatically:
1. Reads the current version from `core/Cargo.toml`
2. Bumps the **patch** version (e.g. `v0.1.3 → v0.1.4`) and pushes a git tag
3. Cross-compiles for:
   - `x86_64-unknown-linux-musl` (Linux, statically linked)
   - `x86_64-pc-windows-gnu` (Windows)
   - `x86_64-apple-darwin` (macOS Intel)
   - `aarch64-apple-darwin` (macOS Apple Silicon)
4. Publishes a GitHub Release with all binaries attached

### Manual version bump (minor / major)

When a non-patch version bump is needed, use the release scripts **before merging**:

```bash
# Bump minor version (0.1.x → 0.2.0)
./scripts/release.sh minor

# Bump major version (0.x.y → 1.0.0)
./scripts/release.sh major

# Set an explicit version
./scripts/release.sh 2.0.0
```

```powershell
# Windows equivalents
.\scripts\release.ps1 minor
.\scripts\release.ps1 2.0.0
```

These scripts update `Cargo.toml` version fields and push a tag, which triggers CI.

---

## 8. Submitting changes

1. **Fork** the repository and create a branch from `main`:
   ```bash
   git checkout -b feat/my-feature
   ```

2. **Make your changes** — keep commits focused; one logical change per commit.

3. **Ensure the build and tests pass:**
   ```bash
   cargo fmt --check
   cargo clippy --workspace -- -D warnings
   cargo test --workspace
   ```

4. **Open a Pull Request** against `main` with:
   - A clear title (e.g. `feat: add redis health-check strategy` or `fix: SSE stream drops on slow consumer`)
   - A short description of what changed and why
   - Steps to test the change locally

### PR guidelines

- Keep PRs small and focused — large refactors should be discussed in an issue first
- Documentation updates are always welcome
- New features should include at least one unit test or an integration test note in the PR description
- Plugin contributions can be submitted as standalone repos and linked in the README

---

## 9. Good first issues

Look for issues labelled [`good first issue`](https://github.com/meliani/Rustboard/issues?q=label%3A%22good+first+issue%22) on GitHub. Ideas that are typically accessible:

- Adding a new CLI subcommand (maps 1:1 to a REST endpoint)
- Adding a new health-check strategy (e.g. `health_grpc`, `health_script`)
- Improving error messages
- Writing a new plugin in any supported language
- Improving the web UI (no build step — edit a single HTML file)
- Adding documentation examples for a specific stack (Kubernetes, Nomad, etc.)


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
├── plugin-openai-tester/   # Bundled example plugin (Extism WASM — tests OpenAI API keys)
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
- [Rust](https://rustup.rs/) stable toolchain (≥ 1.90)
- `wasm32-wasip1` target: `rustup target add wasm32-wasip1`
- SSH access to any remote hosts you want to monitor (optional for local dev)

**Clone and build**
```bash
git clone https://github.com/meliani/Rustboard.git
cd Rustboard
cargo build                                        # builds core, cli, web, plugins
# build + install the bundled WASM plugin:
cargo build -p plugin-openai-tester --target wasm32-wasip1 --release
cp target/wasm32-wasip1/release/plugin_openai_tester.wasm plugins/bin/plugin-openai-tester.wasm
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

Plugins are **WebAssembly modules** (`*.wasm`) loaded at runtime by the host via [Extism](https://extism.org). They are fully sandboxed — no filesystem access, no process spawning — and run on any OS without recompilation.

### How it works

```
POST /plugins/exec  { "name": "my-plugin", "input": { ... } }
        │
        ▼
  core/plugin.rs  ──extism──▶  my-plugin.wasm
                                   │
                              execute(json_string) → json_string
```

The host serialises `input` to a JSON string, passes it to the plugin's exported `execute` function, and returns the raw string result to the caller.

### Plugin contract

Every plugin must export exactly one function:

```
fn execute(input: String) -> String
```

- **Input** — a JSON string (your plugin defines the schema)
- **Output** — a JSON string; by convention include `"ok": true|false`

### Writing a plugin in Rust

**1. Create the crate**

```bash
cargo new --lib plugin-my-plugin
```

**2. Set `Cargo.toml`**

```toml
[package]
name = "plugin-my-plugin"
version = "0.1.0"
edition = "2021"
autobins = false

[lib]
crate-type = ["cdylib"]

[dependencies]
extism-pdk = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

Add the crate to the workspace `Cargo.toml`:

```toml
[workspace]
members = [
    ...
    "plugin-my-plugin",
]
```

**3. Write `src/lib.rs`**

```rust
use extism_pdk::*;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct Input {
    message: String,
}

#[derive(Serialize)]
struct Output {
    ok: bool,
    echo: String,
}

#[plugin_fn]
pub fn execute(raw: String) -> FnResult<String> {
    let input: Input = serde_json::from_str(&raw)
        .map_err(|e| anyhow::anyhow!("invalid input: {}", e))?;

    let out = Output { ok: true, echo: input.message };
    Ok(serde_json::to_string(&out).unwrap())
}
```

**4. Build and install**

```bash
cargo build -p plugin-my-plugin --target wasm32-wasip1 --release
cp target/wasm32-wasip1/release/plugin_my_plugin.wasm plugins/bin/plugin-my-plugin.wasm
```

**5. Invoke**

```bash
curl -s -X POST http://localhost:8080/plugins/exec \
  -H 'Content-Type: application/json' \
  -d '{"name":"plugin-my-plugin","input":{"message":"hello"}}'
# → {"ok":true,"output":"{\"ok\":true,\"echo\":\"hello\"}"}
```

### Writing a plugin in another language

Extism has PDKs for Go, Python, JavaScript, C/C++, Zig, and more. The only requirement is that the compiled `.wasm` exports an `execute` function following the string-in / string-out contract above.

See the [Extism PDK documentation](https://extism.org/docs/concepts/pdk) for language-specific guides.

### Network access

The host grants outbound HTTP access to all hosts by default (`with_allowed_host("*")`). Plugins use `extism_pdk::http::request` — **not** the standard `reqwest`/`fetch` APIs, which are not available in WASI. See `plugin-openai-tester/src/lib.rs` for a complete example.

### Sandboxing defaults

| Capability | Granted by default |
|---|---|
| Outbound HTTP | ✅ (all hosts) |
| Filesystem read/write | ❌ |
| Spawn processes | ❌ |
| Environment variables | ❌ |

To restrict network access to specific hosts, modify `manifest.with_allowed_host(...)` in `core/src/plugin.rs`.

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
