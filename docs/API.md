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
