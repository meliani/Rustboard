# Rustboard — Configuration Reference

Rustboard is configured with two YAML files in the `config/` directory. No environment variables are required for basic operation.

---

## Table of Contents

1. [Services configuration (`services.yaml`)](#1-services-configuration-servicesyaml)
   - [Minimal example](#minimal-example)
   - [Full field reference](#full-field-reference)
   - [Quick commands](#quick-commands)
   - [Stacks and tags](#stacks-and-tags)
   - [Docker-managed services](#docker-managed-services)
2. [Preferences (`preferences.yaml`)](#2-preferences-preferencesyaml)
3. [Environment variables](#3-environment-variables)
4. [Discovered services](#4-discovered-services)
5. [Example configurations](#5-example-configurations)
   - [Systemd services](#systemd-services)
   - [Docker Compose stack](#docker-compose-stack)
   - [Mixed bare-metal + Docker](#mixed-bare-metal--docker)

---

## 1. Services configuration (`services.yaml`)

The service list is loaded from the file path passed as the first argument to `core`. If no argument is given, it defaults to `config/services.example.yaml`.

```bash
./rustboard-core config/services.yaml
# or
cargo run -p core -- config/services.yaml
```

The file must be a **YAML array** of service objects:

```yaml
- id: "service-one"
  name: "Service One"
  host: "10.0.2.10"
  # ... more fields

- id: "service-two"
  # ...
```

### Minimal example

```yaml
- id: "my-api"
  name: "My API"
  host: "10.0.2.10"
```

This is the bare minimum: an `id`, `name`, and `host`. The service will appear in the dashboard with a status of `"unknown"` until a health check resolves.

---

### Full field reference

#### Core identity

| Field | Type | Required | Default | Description |
|---|---|---|---|---|
| `id` | string | ✅ | — | Unique identifier for this service. Used in all API calls. Must be unique across all services (YAML + discovered). |
| `name` | string | ✅ | — | Human-readable display name shown in the dashboard. |
| `host` | string | ✅ | — | Hostname or IP address of the remote machine. |
| `port` | integer | ❌ | `null` | TCP port. Used for HTTP health checks and TCP fallback checks. |
| `ssh_user` | string | ❌ | current user | SSH username for remote connections. If omitted, the system `ssh` client uses its default (typically the current user or `~/.ssh/config`). |

#### Lifecycle commands

These commands are run **on the remote host** via SSH.

| Field | Type | Required | Description |
|---|---|---|---|
| `start_cmd` | string | ❌ | Command to start the service (e.g. `systemctl start my-api`) |
| `stop_cmd` | string | ❌ | Command to stop the service |
| `restart_cmd` | string | ❌ | Command to restart the service |

Example:
```yaml
start_cmd: "systemctl start nginx"
stop_cmd: "systemctl stop nginx"
restart_cmd: "systemctl reload nginx"
```

#### Logging

| Field | Type | Description |
|---|---|---|
| `log_path` | string | Absolute path to a log file on the remote host. Rustboard runs `tail -n <lines> <log_path>`. |
| `log_cmd` | string | Custom command to fetch logs (e.g. `journalctl -u nginx -n 200`). Takes precedence over `log_path` if both are set. |

Only one of `log_path` or `log_cmd` is needed. `log_cmd` is more flexible.

```yaml
# Option A: file path
log_path: "/var/log/nginx/access.log"

# Option B: custom command
log_cmd: "journalctl -u my-api --no-pager -n 500"

# Docker example
log_cmd: "docker logs --tail 200 my-container 2>&1"
```

#### Health checking

Health checks run every 10 seconds. The three strategies are tried in order until one succeeds:

| Field | Type | Strategy | Description |
|---|---|---|---|
| `health_cmd` | string | Strategy 1 (SSH) | Run a remote command; success if output contains `"healthy"`, `"up"`, or `"running"` (case-insensitive) but not `"stopped"`. |
| `health_path` | string | Strategy 2 (HTTP) | HTTP GET to `http://<host>:<port><health_path>`. Success if response is 2xx. Requires `port`. |
| `port` | integer | Strategy 3 (TCP) | TCP connect to `<host>:<port>`. Success if connection established within 1 second. |

If none of these are set, the service status stays `"unknown"`.

```yaml
# SSH health check (Docker)
health_cmd: "docker inspect --format '{{.State.Status}}' my-container"

# HTTP health check
port: 8080
health_path: "/health"

# TCP-only check
port: 5432
```

#### Metadata

| Field | Type | Default | Description |
|---|---|---|---|
| `tags` | string[] | `[]` | Free-form tags for filtering/grouping (e.g. `["nginx", "proxy", "production"]`). |
| `stacks` | string[] | `[]` | Logical stack or project names. A service can belong to multiple stacks. |
| `dependencies` | string[] | `[]` | IDs of services this service depends on. Rendered as edges in the topology view. |

```yaml
tags: ["web", "nginx", "production"]
stacks: ["e-commerce", "monitoring"]
dependencies: ["database", "cache"]
```

#### Container fields

These are populated automatically by Docker discovery. You can also set them manually.

| Field | Type | Description |
|---|---|---|
| `container_name` | string | Docker container name. Used to build `docker exec` commands for quick commands with `in_container: true`. |
| `container_id` | string | Short Docker container ID (informational). |
| `image` | string | Docker image name and tag (informational). |
| `predicted_app_path` | string | Working directory or mount path inside the container (auto-detected by discovery). |

#### Discovery flag

| Field | Type | Default | Description |
|---|---|---|---|
| `discovered` | boolean | `false` | Set to `true` by auto-discovery. Discovered services are refreshed on each rediscovery cycle and can be overwritten. YAML-defined services (`discovered: false`) are never overwritten. |

---

### Quick commands

Quick commands are per-service shell shortcuts that can be executed from the dashboard or CLI.

```yaml
quick_commands:
  - name: "shell"
    cmd: "bash -l"
    description: "Interactive shell"
    in_container: false

  - name: "migrate"
    cmd: "cd /app && php artisan migrate"
    description: "Run database migrations"
    in_container: true    # wraps in "docker exec -it <container_name> ..."

  - name: "logs-error"
    cmd: "tail -n 100 /var/log/app/error.log"
    description: "Show last 100 error log lines"
    in_container: false
```

#### Quick command fields

| Field | Type | Required | Description |
|---|---|---|---|
| `name` | string | ✅ | Identifier used in API calls (`POST /services/quick` → `{"quick": "shell"}`). |
| `cmd` | string | ✅ | Shell command to run. |
| `description` | string | ❌ | Human-readable description shown in the dashboard. |
| `in_container` | boolean | ❌ | If `true` and `container_name` is set, the command is wrapped in `docker exec -it <container_name> <cmd>`. Defaults to `false`. |

> **Note:** `in_container: true` requires the `container_name` field to be set on the service.

---

### Stacks and tags

Use **stacks** for logical grouping by project or environment:

```yaml
stacks: ["glao", "staging"]
```

Use **tags** for technical attributes:

```yaml
tags: ["php", "laravel", "web"]
```

The dashboard can filter and group services by both.

---

### Docker-managed services

For services running in Docker containers on a remote host:

```yaml
- id: "my-laravel-app"
  name: "Laravel App"
  host: "10.0.2.251"
  port: 8000
  ssh_user: "deploy"
  container_name: "my_laravel_app_1"
  start_cmd: "docker start my_laravel_app_1"
  stop_cmd: "docker stop my_laravel_app_1"
  restart_cmd: "docker restart my_laravel_app_1"
  health_cmd: "docker inspect --format '{{.State.Status}}' my_laravel_app_1"
  log_cmd: "docker logs --tail 300 my_laravel_app_1 2>&1"
  tags: ["php", "laravel", "docker"]
  stacks: ["my-project"]
  quick_commands:
    - name: "shell"
      cmd: "bash -l"
      description: "Shell inside container"
      in_container: true
    - name: "artisan-tinker"
      cmd: "php artisan tinker"
      description: "Laravel Tinker REPL"
      in_container: true
    - name: "migrate"
      cmd: "php artisan migrate --force"
      description: "Run migrations"
      in_container: true
```

Alternatively, use **Docker auto-discovery** to generate this config automatically — see `POST /discover` in the [API reference](API.md).

---

## 2. Preferences (`preferences.yaml`)

```yaml
# config/preferences.yaml
show_tooltips: true
theme: dark
```

| Field | Type | Default | Values |
|---|---|---|---|
| `show_tooltips` | boolean | `true` | Show tooltip hints in the dashboard UI |
| `theme` | string | `"dark"` | `"dark"` · `"light"` |

Preferences are reloaded on `POST /config/reload`.

---

## 3. Environment variables

| Variable | Default | Description |
|---|---|---|
| `PLUGIN_DIR` | auto-detected | Override the directory scanned for `*.wasm` plugin files. Default: `bin/` next to the executable (production) or `plugins/bin/` (dev). |
| `RUST_LOG` | (off) | Tracing log filter. Examples: `info`, `debug`, `core=debug,warn`. Uses `tracing-subscriber` format. |

**Examples:**

```bash
# Custom plugin directory
PLUGIN_DIR=/opt/rustboard/plugins ./rustboard-core config/services.yaml

# Enable debug logging
RUST_LOG=debug ./rustboard-core config/services.yaml

# Log only core crate at debug level, everything else at warn
RUST_LOG=core=debug,warn ./rustboard-core config/services.yaml
```

---

## 4. Discovered services

When `POST /discover` is called (or via the background auto-rediscovery worker), discovered containers are written to:

```
config/discovered/<safe_host>.yaml
```

Where `<safe_host>` is the target hostname with non-alphanumeric, non-hyphen characters replaced by `_`.

**Example:** host `10.0.2.10` → `config/discovered/10_0_2_10.yaml`

These files are automatically loaded at startup and on config reload. You can:

- **Edit them** — manually add or change fields; they are loaded just like the main config
- **Delete them** — triggers `POST /discover/forget` behavior; the host's services disappear from the dashboard
- **Commit them** — useful for reproducible deployments where you want discovered state in version control

> Services in discovered files have lower priority than the main config: if the same `id` exists in both, the main config wins.

---

## 5. Example configurations

### Systemd services

```yaml
- id: "nginx"
  name: "Nginx"
  host: "10.0.2.10"
  port: 80
  ssh_user: "admin"
  start_cmd: "sudo systemctl start nginx"
  stop_cmd: "sudo systemctl stop nginx"
  restart_cmd: "sudo systemctl reload nginx"
  health_path: "/"
  log_cmd: "sudo journalctl -u nginx --no-pager -n 200"
  tags: ["web", "proxy"]

- id: "postgres"
  name: "PostgreSQL"
  host: "10.0.2.200"
  port: 5432
  ssh_user: "admin"
  start_cmd: "sudo systemctl start postgresql"
  stop_cmd: "sudo systemctl stop postgresql"
  restart_cmd: "sudo systemctl restart postgresql"
  log_cmd: "sudo journalctl -u postgresql --no-pager -n 200"
  tags: ["database", "postgres"]
  quick_commands:
    - name: "psql"
      cmd: "psql -U postgres"
      description: "Open PostgreSQL shell"
      in_container: false
```

---

### Docker Compose stack

```yaml
- id: "frontend"
  name: "React Frontend"
  host: "10.0.2.251"
  port: 3000
  ssh_user: "deploy"
  container_name: "myapp_frontend_1"
  start_cmd: "docker start myapp_frontend_1"
  stop_cmd: "docker stop myapp_frontend_1"
  restart_cmd: "docker restart myapp_frontend_1"
  health_cmd: "docker inspect --format '{{.State.Status}}' myapp_frontend_1"
  log_cmd: "docker logs --tail 200 myapp_frontend_1 2>&1"
  stacks: ["myapp"]
  tags: ["react", "frontend"]

- id: "backend"
  name: "Node.js API"
  host: "10.0.2.251"
  port: 4000
  ssh_user: "deploy"
  container_name: "myapp_backend_1"
  start_cmd: "docker start myapp_backend_1"
  stop_cmd: "docker stop myapp_backend_1"
  restart_cmd: "docker restart myapp_backend_1"
  health_cmd: "docker inspect --format '{{.State.Status}}' myapp_backend_1"
  log_cmd: "docker logs --tail 200 myapp_backend_1 2>&1"
  stacks: ["myapp"]
  tags: ["node", "api"]
  dependencies: ["database"]
  quick_commands:
    - name: "shell"
      cmd: "sh"
      in_container: true
    - name: "migrate"
      cmd: "node migrate.js"
      description: "Run DB migrations"
      in_container: true

- id: "database"
  name: "PostgreSQL"
  host: "10.0.2.251"
  port: 5432
  ssh_user: "deploy"
  container_name: "myapp_db_1"
  start_cmd: "docker start myapp_db_1"
  stop_cmd: "docker stop myapp_db_1"
  restart_cmd: "docker restart myapp_db_1"
  log_cmd: "docker logs --tail 200 myapp_db_1 2>&1"
  stacks: ["myapp"]
  tags: ["postgres", "database"]
```

---

### Mixed bare-metal + Docker

```yaml
# Bare-metal service
- id: "load-balancer"
  name: "HAProxy"
  host: "10.0.2.1"
  port: 80
  ssh_user: "ops"
  restart_cmd: "sudo systemctl reload haproxy"
  health_path: "/stats"
  tags: ["haproxy", "lb"]

# Docker services on two separate hosts
- id: "api-eu"
  name: "API (EU)"
  host: "10.0.2.10"
  port: 8080
  ssh_user: "deploy"
  container_name: "api_eu"
  restart_cmd: "docker restart api_eu"
  health_cmd: "docker inspect --format '{{.State.Status}}' api_eu"
  tags: ["api", "eu"]
  stacks: ["api"]
  dependencies: ["load-balancer"]

- id: "api-us"
  name: "API (US)"
  host: "10.0.2.20"
  port: 8080
  ssh_user: "deploy"
  container_name: "api_us"
  restart_cmd: "docker restart api_us"
  health_cmd: "docker inspect --format '{{.State.Status}}' api_us"
  tags: ["api", "us"]
  stacks: ["api"]
  dependencies: ["load-balancer"]
```
