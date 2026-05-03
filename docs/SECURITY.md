# Rustboard — Security Model

This document describes Rustboard's security architecture, the threat model it is designed for, and how to harden a deployment for production use.

---

## Table of Contents

1. [Intended deployment context](#1-intended-deployment-context)
2. [SSH security model](#2-ssh-security-model)
3. [WebAssembly plugin sandboxing](#3-webassembly-plugin-sandboxing)
4. [API security](#4-api-security)
5. [Hardening recommendations](#5-hardening-recommendations)
6. [Reporting a vulnerability](#6-reporting-a-vulnerability)

---

## 1. Intended deployment context

Rustboard is designed for **trusted private networks** — home labs, internal LAN environments, and private cloud infrastructure where the dashboard host and all monitored hosts are behind a firewall or VPN.

**It is not designed for:**
- Public internet exposure without a reverse proxy with authentication
- Multi-tenant environments where different users have different access levels
- Environments where the `core` binary is run by an untrusted user

If you expose Rustboard on a public IP, you **must** place it behind a reverse proxy (e.g. Nginx, Caddy, Traefik) with at least HTTP Basic Auth or OAuth.

---

## 2. SSH security model

### How Rustboard uses SSH

All remote operations (service commands, log fetching, health checks, Docker discovery) run the system `ssh` binary as a subprocess. Rustboard does **not** implement its own SSH client and does **not** store or handle SSH credentials.

### SSH options applied globally

```
BatchMode=yes
```
Disables interactive prompts. If the SSH connection requires a password or an unrecognised host key, the command fails immediately with an error rather than hanging. This prevents credential leakage through interactive prompts.

```
ConnectTimeout=10
```
Hard 10-second timeout on the TCP handshake. Unreachable hosts fail fast.

```
ServerAliveInterval=15
ServerAliveCountMax=3
```
Send keep-alive probes every 15 seconds; drop the connection after 3 unanswered probes (~45 seconds of silence). This ensures dead SSH connections are cleaned up even without OS-level TCP keepalives.

```
StrictHostKeyChecking=accept-new
```
**Auto-accepts new host keys** (first connection to a host) but **rejects changed keys**. This is safe for private infrastructure where you control all hosts, while still detecting key changes that could indicate a MITM attack or a host rebuild.

> If you manage a fleet of ephemeral machines (e.g. cloud VMs that get new SSH host keys on each provision), consider setting `StrictHostKeyChecking=no` in your `~/.ssh/config` for the specific subnets, and use SSH CA certificates instead.

### Credential storage

Rustboard stores no SSH credentials. Authentication relies entirely on:

1. **SSH agent** (`SSH_AUTH_SOCK`) — the recommended approach
2. **Key files** (`~/.ssh/id_ed25519`, `~/.ssh/id_rsa`, etc.)
3. **`~/.ssh/config`** entries (e.g. per-host `IdentityFile` directives)

### Command injection risk

Service commands (`start_cmd`, `stop_cmd`, `restart_cmd`, `health_cmd`, `log_cmd`, quick commands) are passed verbatim to the remote shell via SSH. They are defined in your YAML config — **they are not user-supplied at runtime via the API**.

If you accept service config from untrusted users, validate and sanitise all command fields before writing them to disk. The API endpoints accept `id` and `cmd` identifiers, not raw shell strings, for lifecycle commands.

---

## 3. WebAssembly plugin sandboxing

### Sandbox boundary

Plugins are WebAssembly modules executed inside the [Extism](https://extism.org) runtime (which uses [Wasmtime](https://wasmtime.dev/) under the hood). The WASM sandbox enforces strict isolation:

| Capability | Default | Notes |
|---|---|---|
| Filesystem read | ❌ Denied | |
| Filesystem write | ❌ Denied | |
| Subprocess spawning | ❌ Denied | Not available in WASM |
| Environment variable access | ❌ Denied | |
| Outbound HTTP | ✅ Allowed (all hosts) | Configurable — see below |
| System clock | ✅ Allowed | WASI standard |
| Cryptographic randomness | ✅ Allowed | WASI standard |

A compromised or malicious plugin **cannot**:
- Read or write files on the server
- Execute system commands
- Access environment variables or credentials

A plugin **can**:
- Make outbound HTTP requests to any host (by default)
- Consume CPU and memory up to process limits
- Return arbitrary data in its output string

### Restricting plugin network access

If you want to restrict which hosts a plugin can reach, modify `core/src/plugin.rs`:

```rust
// Current default: allow all hosts
let manifest = extism::Manifest::new([wasm]).with_allowed_host("*");

// Restrict to a specific domain
let manifest = extism::Manifest::new([wasm]).with_allowed_host("api.openai.com");

// Allow multiple domains
let manifest = extism::Manifest::new([wasm])
    .with_allowed_host("api.openai.com")
    .with_allowed_host("api.anthropic.com");

// No network access at all
let manifest = extism::Manifest::new([wasm]);  // no with_allowed_host call
```

### Plugin source trust

Only load `.wasm` files that you built yourself or obtained from a trusted source. A malicious WASM file could:
- Exfiltrate secrets passed as `input` via outbound HTTP
- Perform SSRF (Server-Side Request Forgery) against internal services accessible from the server's network

**Best practice:** Never install plugins from untrusted sources. Build your own or inspect the source before compiling.

### Path traversal protection

The `POST /plugins/exec` endpoint rejects plugin names containing `/`, `\`, or `..`:

```rust
if body.name.contains('/') || body.name.contains('\\') || body.name.contains("..") {
    return axum::Json(json!({"ok": false, "error": "invalid plugin name"}));
}
```

This prevents an attacker from loading arbitrary WASM files from outside the plugin directory.

---

## 4. API security

### No built-in authentication

Rustboard does not include built-in authentication or authorisation. Every client that can reach port 8080 can:
- View all service configurations (including host IPs and SSH usernames)
- Execute start/stop/restart commands on any service
- Execute arbitrary quick commands over SSH
- Load and execute WASM plugins

### Protecting the API

For any deployment accessible outside of `localhost`, use one of:

**Option A: Reverse proxy with HTTP Basic Auth (Nginx)**
```nginx
server {
    listen 443 ssl;
    server_name dashboard.internal.example.com;

    auth_basic "Rustboard";
    auth_basic_user_file /etc/nginx/.htpasswd;

    location / {
        proxy_pass http://127.0.0.1:8080;
        proxy_http_version 1.1;
        proxy_set_header Upgrade $http_upgrade;
        proxy_set_header Connection "upgrade";
        proxy_set_header Host $host;
        # Required for SSE
        proxy_buffering off;
        proxy_cache off;
        proxy_read_timeout 86400s;
    }
}
```

**Option B: VPN / WireGuard**

Place Rustboard on a WireGuard or OpenVPN network. The dashboard binds to `0.0.0.0:8080` but is only reachable by peers on the VPN.

**Option C: SSH tunnel**

For ad-hoc access without exposing the port at all:
```bash
ssh -L 8080:localhost:8080 user@server-running-rustboard
# Then open http://localhost:8080 in your browser
```

### Binding to localhost only

If you want Rustboard to be accessible only from the local machine, set the bind address to `127.0.0.1`. Currently, the bind address is hardcoded to `0.0.0.0:8080` in `core/src/main.rs`. To restrict it:

```rust
// core/src/main.rs — change the bind address
let listener = tokio::net::TcpListener::bind("127.0.0.1:8080").await?;
```

---

## 5. Hardening recommendations

| Priority | Recommendation |
|---|---|
| 🔴 Critical | Place behind a reverse proxy with authentication if accessible outside localhost |
| 🔴 Critical | Use SSH key-based authentication; disable password auth on all managed hosts |
| 🟠 High | Use a dedicated SSH user with limited sudo permissions on managed hosts |
| 🟠 High | Restrict plugin network access to specific domains in `plugin.rs` |
| 🟠 High | Only install WASM plugins you built from reviewed source code |
| 🟡 Medium | Use `StrictHostKeyChecking=yes` in production and pre-populate `known_hosts` |
| 🟡 Medium | Use SSH CA certificates for host authentication on ephemeral infrastructure |
| 🟡 Medium | Run the `core` binary as a non-root user with minimum required permissions |
| 🟢 Low | Bind to `127.0.0.1` and use an SSH tunnel for remote access |

### Minimal SSH user permissions example

Create a dedicated user on managed hosts:

```bash
# On the managed host
useradd -m -s /bin/bash rustboard
# Grant only the commands Rustboard needs
echo "rustboard ALL=(ALL) NOPASSWD: /usr/bin/systemctl start *, /usr/bin/systemctl stop *, /usr/bin/systemctl restart *" \
  >> /etc/sudoers.d/rustboard
```

This limits the blast radius if the Rustboard server is compromised.

---

## 6. Reporting a vulnerability

If you discover a security vulnerability in Rustboard, please report it privately:

1. Do **not** open a public GitHub issue
2. Email the maintainer directly (see the GitHub profile) or use [GitHub's private vulnerability reporting](https://docs.github.com/en/code-security/security-advisories/guidance-on-reporting-and-writing/privately-reporting-a-security-vulnerability)
3. Include a description of the vulnerability, steps to reproduce, and any suggested mitigations

We will acknowledge the report within 72 hours and aim to release a fix within 14 days for critical issues.
