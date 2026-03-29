# RUSH (Rust Dash)

Lightweight Rust-based local LAN microservices dashboard (CLI + web). **RUSH** is an open-source contribution from **International Micro Services (IMS)** to the developer community.

This repository contains:

Quick start:

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
.\rush-core.exe config\services.example.yaml

# Manage using the CLI
.\rush-cli.exe list
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
# cd /mnt/c/code/dev-projects/RUST-MICROSERVICES-DASHBOARD
# cargo build --workspace
# cargo run -p core -- config/services.example.yaml
```
```
