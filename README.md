# Rust Microservices Dashboard

Lightweight Rust-based local LAN microservices dashboard (CLI + web). This repository contains an initial scaffold: `core` (HTTP API + service manager), `cli` (command-line client), `web` (static assets), and `plugins` (plugin trait).

Quick start:

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
