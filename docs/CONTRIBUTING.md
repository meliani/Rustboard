# Contributing

Thank you for your interest in contributing. This project aims to remain small and approachable. For now:

- Build with `cargo build --workspace`.
- Run the core server with `cargo run -p core -- config/services.example.yaml`.
- The `cli` crate is a minimal example showing how to call the HTTP API.
- Open issues and PRs are welcome. For plugins, add a crate under `plugins/` or implement the `DashboardPlugin` trait.
