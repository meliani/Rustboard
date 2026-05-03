# Rustboard — Plugin Developer Guide

Plugins are the primary extension point in Rustboard. They are **WebAssembly modules** (`.wasm` files) that the server loads at runtime using the [Extism](https://extism.org) host library. Because plugins run inside a WASM sandbox, they are:

- **Portable** — compile once, run on any OS Rustboard supports
- **Safe** — no filesystem access, no subprocess spawning by default
- **Language-agnostic** — write in Rust, Go, Python, JavaScript, C, Zig, or any language with a WASI-compatible compiler

---

## Table of Contents

1. [How the Plugin System Works](#1-how-the-plugin-system-works)
2. [Plugin Contract](#2-plugin-contract)
3. [Writing a Plugin in Rust](#3-writing-a-plugin-in-rust)
4. [Writing a Plugin in Go](#4-writing-a-plugin-in-go)
5. [Writing a Plugin in JavaScript / TypeScript](#5-writing-a-plugin-in-javascript--typescript)
6. [Writing a Plugin in Python](#6-writing-a-plugin-in-python)
7. [HTTP / Network Access from Plugins](#7-http--network-access-from-plugins)
8. [Installing Plugins](#8-installing-plugins)
9. [Invoking Plugins](#9-invoking-plugins)
10. [Plugin Patterns & Best Practices](#10-plugin-patterns--best-practices)
11. [Sandboxing Reference](#11-sandboxing-reference)
12. [Reference Plugin: `plugin-openai-tester`](#12-reference-plugin-plugin-openai-tester)
13. [Troubleshooting](#13-troubleshooting)

---

## 1. How the Plugin System Works

```
POST /plugins/exec
{
  "name": "my-plugin",      ← stem of the .wasm file in the plugin directory
  "input": { ... }          ← any JSON value
}
        │
        ▼
core/src/plugin.rs
  extism::Plugin::new(plugins/bin/my-plugin.wasm)
        │
        ▼
  plugin.call("execute", json_string)
        │
        ▼
  my-plugin.wasm
    fn execute(input: String) -> String
        │
        ▼
  raw_output_string
        │
        ▼
API response: { "ok": true, "output": "<raw_output_string>" }
```

Each invocation:
1. Loads the `.wasm` file from disk
2. Creates a **fresh plugin instance** (no shared state between calls)
3. Serialises `input` to a JSON string
4. Calls the exported `execute` function
5. Returns the raw string result as the `output` field

> **Note:** A fresh instance is created for every call — plugins are stateless by design. If you need to persist state between calls, write it to an external store (e.g. a database or file) via HTTP.

---

## 2. Plugin Contract

Every plugin **must** export exactly one function:

```
fn execute(input: String) -> String
```

- **`input`** — A JSON string whose schema is entirely up to the plugin. The host passes the `input` field from the API request, serialised with `serde_json`. If `input` is omitted in the request body, an empty string `""` is passed.
- **Return value** — A JSON string. By convention, always include an `"ok"` boolean:

```json
// Success
{ "ok": true, "result": "..." }

// Failure
{ "ok": false, "error": "human-readable error message" }
```

The host does **not** parse or validate your output — it is returned verbatim as the `output` field in the API response.

---

## 3. Writing a Plugin in Rust

### Prerequisites

```bash
rustup target add wasm32-wasip1
```

### Step 1 — Create the crate

```bash
cargo new --lib plugin-my-plugin
cd plugin-my-plugin
```

### Step 2 — Configure `Cargo.toml`

```toml
[package]
name = "plugin-my-plugin"
version = "0.1.0"
edition = "2021"
autobins = false

[lib]
crate-type = ["cdylib"]    # Required: produces a .wasm shared library

[dependencies]
extism-pdk = "1"
serde = { version = "1", features = ["derive"] }
serde_json = "1"
```

If your plugin is part of the Rustboard workspace, add it to the workspace `Cargo.toml`:

```toml
# Cargo.toml (workspace root)
[workspace]
members = [
    "core", "cli", "web", "plugins",
    "plugin-my-plugin",   # ← add this
]
# Keep it out of default-members so cargo build --workspace doesn't try
# to compile it for the native target:
default-members = ["core", "cli", "web", "plugins"]
```

### Step 3 — Write `src/lib.rs`

```rust
use extism_pdk::*;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct Input {
    message: String,
}

#[derive(Serialize)]
#[serde(untagged)]
enum Output {
    Ok { ok: bool, echo: String },
    Err { ok: bool, error: String },
}

#[plugin_fn]
pub fn execute(raw: String) -> FnResult<String> {
    // Parse input
    let input: Input = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            return Ok(serde_json::to_string(&Output::Err {
                ok: false,
                error: format!("invalid input: {}", e),
            })
            .unwrap());
        }
    };

    // Do work
    let out = Output::Ok {
        ok: true,
        echo: input.message.to_uppercase(),
    };
    Ok(serde_json::to_string(&out).unwrap())
}
```

### Step 4 — Build

```bash
cargo build -p plugin-my-plugin --target wasm32-wasip1 --release
```

The compiled module lands at:
```
target/wasm32-wasip1/release/plugin_my_plugin.wasm
```

> Cargo converts hyphens to underscores in the output filename — `plugin-my-plugin` → `plugin_my_plugin.wasm`.

### Step 5 — Install

```bash
# Copy and rename to match the expected plugin name
cp target/wasm32-wasip1/release/plugin_my_plugin.wasm plugins/bin/plugin-my-plugin.wasm
```

### Step 6 — Invoke

```bash
curl -s -X POST http://localhost:8080/plugins/exec \
  -H 'Content-Type: application/json' \
  -d '{"name":"plugin-my-plugin","input":{"message":"hello world"}}'
```

```json
{
  "ok": true,
  "output": "{\"ok\":true,\"echo\":\"HELLO WORLD\"}"
}
```

---

## 4. Writing a Plugin in Go

### Prerequisites

```bash
# Install TinyGo (supports wasm32-wasi)
# https://tinygo.org/getting-started/install/
```

### `main.go`

```go
package main

import (
    "encoding/json"

    "github.com/extism/go-pdk"
)

type Input struct {
    Message string `json:"message"`
}

type Output struct {
    Ok    bool   `json:"ok"`
    Echo  string `json:"echo,omitempty"`
    Error string `json:"error,omitempty"`
}

//export execute
func execute() int32 {
    raw := pdk.InputString()

    var input Input
    if err := json.Unmarshal([]byte(raw), &input); err != nil {
        out, _ := json.Marshal(Output{Ok: false, Error: err.Error()})
        pdk.OutputString(string(out))
        return 0
    }

    result := Output{Ok: true, Echo: input.Message}
    out, _ := json.Marshal(result)
    pdk.OutputString(string(out))
    return 0
}

func main() {}
```

### Build

```bash
tinygo build -target wasi -o plugins/bin/plugin-my-go-plugin.wasm main.go
```

---

## 5. Writing a Plugin in JavaScript / TypeScript

Extism provides a JS/TS PDK that compiles to WASM via [Javy](https://github.com/bytecodealliance/javy).

### Install the PDK

```bash
npm install @extism/extism-pdk
```

### `plugin.js`

```js
function execute() {
  const raw = Host.inputString();
  let input;
  try {
    input = JSON.parse(raw);
  } catch (e) {
    Host.outputString(JSON.stringify({ ok: false, error: `invalid JSON: ${e.message}` }));
    return;
  }
  Host.outputString(JSON.stringify({ ok: true, echo: (input.message || "").toUpperCase() }));
}

module.exports = { execute };
```

### Build

```bash
# Using the Extism JS PDK build tool
extism-js plugin.js -o plugins/bin/plugin-my-js-plugin.wasm
```

---

## 6. Writing a Plugin in Python

Extism's Python PDK uses [py2wasm](https://github.com/nicowillis/py2wasm) or the Extism compiler.

```python
import extism
import json

@extism.plugin_fn
def execute():
    raw = extism.input_str()
    try:
        data = json.loads(raw)
    except Exception as e:
        extism.output_str(json.dumps({"ok": False, "error": str(e)}))
        return

    extism.output_str(json.dumps({"ok": True, "echo": data.get("message", "").upper()}))
```

Compile with the Extism Python PDK toolchain. See [extism/python-pdk](https://github.com/extism/python-pdk) for current instructions.

---

## 7. HTTP / Network Access from Plugins

Standard HTTP libraries (`reqwest`, `fetch`, `urllib`) are **not available** inside the WASM sandbox. Use the Extism PDK's built-in HTTP API instead.

### Rust example

```rust
use extism_pdk::*;

#[plugin_fn]
pub fn execute(raw: String) -> FnResult<String> {
    let req = HttpRequest::new("https://api.example.com/data")
        .with_method("GET")
        .with_header("Authorization", "Bearer my-token")
        .with_header("Accept", "application/json");

    let resp = http::request::<()>(&req, None)?;
    let status = resp.status_code();
    let body = String::from_utf8_lossy(&resp.body()).to_string();

    Ok(serde_json::json!({
        "ok": status >= 200 && status < 300,
        "status": status,
        "body": body
    }).to_string())
}
```

### POST request with a body

```rust
#[derive(Serialize)]
struct Payload { key: String, value: String }

let payload = Payload { key: "x".into(), value: "y".into() };
let req = HttpRequest::new("https://api.example.com/submit")
    .with_method("POST")
    .with_header("Content-Type", "application/json");

let resp = http::request(&req, Some(payload))?;
```

### Host-side network policy

By default, the host grants outbound HTTP access to **all hosts** (`with_allowed_host("*")`). To restrict a plugin to specific domains, modify `core/src/plugin.rs`:

```rust
// Allow only api.openai.com
let manifest = extism::Manifest::new([wasm])
    .with_allowed_host("api.openai.com");
```

---

## 8. Installing Plugins

Plugins are `.wasm` files placed in the **plugin directory**:

| Context | Default plugin directory |
|---|---|
| Installed binary | `bin/` next to the `core` executable |
| `cargo run` (dev) | `plugins/bin/` relative to workspace root |
| Override | Set `PLUGIN_DIR` environment variable |

```bash
# Override example
PLUGIN_DIR=/opt/rustboard/plugins cargo run -p core -- config/services.yaml
```

### Naming convention

The file stem becomes the plugin's `name` in the API:

```
plugins/bin/plugin-openai-tester.wasm
               └─────────────────── name = "plugin-openai-tester"
```

Rustboard discovers all `*.wasm` files in the directory at each request — no restart required after adding a new plugin.

---

## 9. Invoking Plugins

### List installed plugins

```bash
curl http://localhost:8080/plugins
# → {"ok":true,"plugins":["plugin-openai-tester","plugin-my-plugin"]}
```

### Execute a plugin

```bash
curl -s -X POST http://localhost:8080/plugins/exec \
  -H 'Content-Type: application/json' \
  -d '{
    "name": "plugin-my-plugin",
    "input": { "message": "hello" }
  }'
```

```json
{
  "ok": true,
  "output": "{\"ok\":true,\"echo\":\"HELLO\"}"
}
```

The `output` field is the raw string returned by `execute`. Parse it as JSON to access plugin-specific fields:

```js
const response = await fetch('/plugins/exec', { /* ... */ }).then(r => r.json());
const pluginResult = JSON.parse(response.output);
console.log(pluginResult.echo); // "HELLO"
```

### From the CLI

```bash
# The CLI doesn't have a built-in plugin subcommand yet;
# use curl or HTTPie directly
```

---

## 10. Plugin Patterns & Best Practices

### Always validate input

Never assume the input JSON is well-formed. Return a clear error on parse failure:

```rust
let input: MyInput = match serde_json::from_str(&raw) {
    Ok(v) => v,
    Err(e) => return Ok(serde_json::json!({"ok":false,"error":format!("bad input: {}",e)}).to_string()),
};
```

### Use `#[serde(untagged)]` for multi-variant output

```rust
#[derive(Serialize)]
#[serde(untagged)]
enum Output {
    Success { ok: bool, data: MyData },
    Failure { ok: bool, error: String },
}
```

This produces clean JSON without a `"type"` discriminant field.

### Keep plugins focused

One plugin = one responsibility. A plugin that tests API keys shouldn't also parse log files. Small, focused plugins are easier to test, version, and replace.

### Version your output schema

If you change the output schema in a breaking way, bump the plugin name or add a `version` field to the output so callers can adapt:

```json
{ "ok": true, "version": 2, "result": { ... } }
```

### Test without the server

Build a native binary alongside the WASM for local testing:

```toml
# Cargo.toml
[[bin]]
name = "plugin-my-plugin-test"
path = "src/main.rs"          # thin wrapper that reads stdin, calls execute(), prints stdout
```

```bash
echo '{"message":"hello"}' | ./target/debug/plugin-my-plugin-test
```

---

## 11. Sandboxing Reference

| Capability | Default | How to change |
|---|---|---|
| Outbound HTTP — all hosts | ✅ Allowed | Restrict: `manifest.with_allowed_host("api.example.com")` in `plugin.rs` |
| Outbound HTTP — specific host | ❌ by default if restricted | Add with `with_allowed_host` |
| Filesystem read | ❌ | Grant: `manifest.with_allowed_path("/data", "/data")` |
| Filesystem write | ❌ | Grant: `manifest.with_allowed_path("/tmp", "/tmp")` |
| Subprocess spawning | ❌ | Not available — WASM does not support this |
| Environment variables | ❌ | Grant: `manifest.with_config_key("MY_KEY", "value")` |
| System clock | ✅ WASI | Standard `std::time` works in WASI |
| Cryptographic randomness | ✅ WASI | Standard `rand` works in WASI |

---

## 12. Reference Plugin: `plugin-openai-tester`

**Source:** `plugin-openai-tester/src/lib.rs`

Tests whether an OpenAI-compatible API key is valid by calling the `/models` endpoint.

### Input schema

```json
{
  "api_key": "sk-...",
  "base_url": "https://api.openai.com/v1"
}
```

| Field | Type | Required | Default |
|---|---|---|---|
| `api_key` | string | ✅ | — |
| `base_url` | string | ❌ | `https://api.openai.com/v1` |

Compatible with any OpenAI-compatible API: Azure OpenAI, Ollama, Groq, Together AI, Mistral, etc.

### Output schema

```json
// Key is valid
{ "ok": true, "valid": true, "models": ["gpt-4o", "gpt-4-turbo", ...] }

// Key is invalid (401/403)
{ "ok": true, "valid": false, "error": "Incorrect API key provided" }

// Input parsing failed or network error
{ "ok": false, "error": "..." }
```

### Build and install

```bash
# From workspace root
cargo build -p plugin-openai-tester --target wasm32-wasip1 --release
cp target/wasm32-wasip1/release/plugin_openai_tester.wasm \
   plugins/bin/plugin-openai-tester.wasm

# Or use the helper script:
./scripts/install-plugin-openai-tester.sh --release   # Linux/macOS
.\scripts\install-plugin-openai-tester.ps1 -Release   # Windows
```

### Example invocation

```bash
# Test OpenAI key
curl -s -X POST http://localhost:8080/plugins/exec \
  -H 'Content-Type: application/json' \
  -d '{"name":"plugin-openai-tester","input":{"api_key":"sk-..."}}'

# Test a local Ollama instance
curl -s -X POST http://localhost:8080/plugins/exec \
  -H 'Content-Type: application/json' \
  -d '{"name":"plugin-openai-tester","input":{"api_key":"ollama","base_url":"http://localhost:11434/v1"}}'
```

---

## 13. Troubleshooting

### `plugin not found`

- Check the file exists: `ls plugins/bin/*.wasm`
- Confirm the `name` in the request matches the **file stem** (without `.wasm`)
- Check if `PLUGIN_DIR` is set and points to the right directory

### `loading WASM plugin` error

- Ensure the file is a valid WASM module: `wasm-validate plugins/bin/my-plugin.wasm`
- Confirm it was compiled for `wasm32-wasip1` (not `wasm32-unknown-unknown`)
- Rebuild: `cargo build -p plugin-my-plugin --target wasm32-wasip1 --release`

### `executing plugin function` error — `function execute not found`

- The WASM module must export a function named exactly `execute`
- In Rust: ensure `#[plugin_fn]` is applied to the `execute` function and the crate type is `cdylib`
- Run `wasm-objdump -x plugins/bin/my-plugin.wasm | grep Export` to inspect exports

### `network error` from HTTP calls

- By default all outbound hosts are allowed — verify the URL is correct
- If you restricted hosts with `with_allowed_host`, ensure the target host is in the allowlist
- Plugins use Extism's `http::request` — standard HTTP libraries are not available inside WASM

### Plugin returns empty output

- The `execute` function must return a non-empty string — an empty return is treated as success with an empty `output` field
- Add a fallback: `Ok("{}".to_string())` at minimum

### Performance: slow cold start

Each call creates a fresh WASM instance (JIT compilation on first call may add ~50–200 ms depending on module size). If latency matters, consider keeping module size small (avoid large embedded assets).
