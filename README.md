# Rustboard

**Rustboard** is a lightweight, open-source microservices dashboard built in Rust — combining a real-time web UI, a CLI, and an extensible plugin system to monitor and manage services across your LAN over SSH.

Open-source contribution by **International Micro Services (IMS)**.

## Features

- Real-time service health monitoring (SSE + WebSocket)
- Start / stop / restart services via SSH
- Log tailing and custom quick commands
- Docker service auto-discovery
- Plugin system — installable extensions (e.g. `plugin-openai-tester`)
- Network topology view
- Dark-themed web UI (vanilla JS + HTMX, no framework)
- CLI for scripting and automation

## Quick Start

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

```powershell
# wsl
# cd /mnt/c/code/dev-projects/Rustboard
# cargo build --workspace
# cargo run -p core -- config/services.example.yaml
```
```
