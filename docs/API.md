# Core API

Base URL: `http://localhost:8080`

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
