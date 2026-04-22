# Rustboard

**Rustboard** is a lightweight, open-source microservices dashboard built in Rust — combining a real-time web UI, a CLI, and an extensible plugin system to monitor and manage services across your LAN over SSH.

## Features

- Real-time service health monitoring (SSE + WebSocket)
- Start / stop / restart services via SSH
- Log tailing and custom quick commands
- Docker service auto-discovery
- Plugin system — installable extensions (e.g. `plugin-openai-tester`)
- Network topology view
- Dark-themed web UI (vanilla JS + HTMX, no framework)
- CLI for scripting and automation

## Included Plugin

This repository includes a sample plugin: `plugin-openai-tester` (source in `plugin-openai-tester/`). The plugin verifies OpenAI-compatible API keys by querying the `/models` endpoint and returns a JSON result suitable for the dashboard plugin contract.

Build and install the plugin into `plugins/bin/` using the included scripts:

**PowerShell**
```powershell
.\scripts\install-plugin-openai-tester.ps1 -Release
```

**Bash**
```bash
./scripts/install-plugin-openai-tester.sh --release
```

Usage via the dashboard API:

```json
POST /plugins/exec
{ "name": "plugin-openai-tester", "input": { "api_key": "sk-..." } }
```

You can also run the plugin locally for quick testing:

```bash
echo '{"api_key":"sk-invalid-key"}' | ./target/debug/plugin-openai-tester
```

## Installation

### Download a prebuilt release (recommended)

**Linux / macOS**
```bash
curl -fsSL https://raw.githubusercontent.com/meliani/Rustboard/main/install.sh | bash
```

**Windows (PowerShell)**
```powershell
irm https://raw.githubusercontent.com/meliani/Rustboard/main/install.ps1 | iex
```

The installer auto-detects your platform, downloads the latest binaries from the [GitHub Releases page](https://github.com/meliani/Rustboard/releases), and adds them to your PATH.

> **Custom install directory**
> ```bash
> # Linux / macOS
> curl -fsSL .../install.sh | bash -s -- --dir ~/.local/bin
> # Windows
> $env:RUSTBOARD_DIR = "$HOME\.rustboard\bin"; irm .../install.ps1 | iex
> ```

---

## Build from Source

### Building a Local Executable

1. Generate direct native binaries using the build scripts:

**On Windows (PowerShell):**
```powershell
.\scripts\build-local.ps1
```

**On Linux/WSL (Bash):**
```bash
sh ./scripts/build-local.sh
```

2. Execute the app directly from the generated binaries:

```powershell
# Start the core dashboard server
.\rustboard-core.exe config\services.example.yaml

# Manage using the CLI
.\rustboard-cli.exe list
```
*(Note: Omit `.exe` on Linux/WSL)*

---

### Running via Cargo (Development)

1. Build the workspace:

```powershell
cargo build --workspace
```

2. Run the core server (serves static UI and API):

```powershell
cargo run -p core -- config/services.example.yaml
```

3. Use the CLI to list services:

```powershell
cargo run -p cli -- list

Run on Windows / using WSL
-------------------------
This project targets a Linux environment. On Windows we recommend running the workspace inside WSL (Windows Subsystem for Linux). A helper script is provided at `scripts/ensure-wsl.ps1` to run commands inside WSL from PowerShell.

Example (PowerShell):

```powershell
.\scripts\ensure-wsl.ps1 cargo build --workspace
.\scripts\ensure-wsl.ps1 cargo run -p core -- config/services.example.yaml
```

If you prefer to run commands manually in WSL:

```bash
# wsl
# cd /mnt/c/code/dev-projects/Rustboard
# cargo build --workspace
# cargo run -p core -- config/services.example.yaml
```

---

## Releases

Every push to `main` automatically builds and publishes a new release:

1. Patch version is bumped automatically (`v0.1.3 → v0.1.4`)
2. Binaries are built for Linux, Windows, and macOS (x86_64 + Apple Silicon)
3. A [GitHub Release](https://github.com/meliani/Rustboard/releases) is published with all binaries attached

For a **minor or major** version bump, use the release scripts:

```powershell
# Windows
.\scripts\release.ps1 minor
.\scripts\release.ps1 1.0.0
```
```bash
# Linux / macOS
./scripts/release.sh minor
./scripts/release.sh 1.0.0
```

---

## Contributing

See [docs/CONTRIBUTING.md](docs/CONTRIBUTING.md) for the full developer guide, including:

- Project structure
- Local dev setup and running tests
- Writing and installing plugins
- The release process in detail
- How to submit a PR

