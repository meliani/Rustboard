/// plugin-openai-tester (Extism WASM plugin)
///
/// Plugin protocol:
///   input  → JSON: { "api_key": "sk-...", "base_url": "https://api.openai.com/v1" }
///            `base_url` is optional; defaults to https://api.openai.com/v1
///            Any OpenAI-compatible endpoint works (Azure, Ollama, Groq, etc.)
///   output → JSON: { "ok": true,  "valid": true,  "models": ["gpt-4o", ...] }
///          | JSON: { "ok": true,  "valid": false, "error": "..." }
///          | JSON: { "ok": false, "error": "..." }   ← bad input / network fail
///
/// Build:   cargo build -p plugin-openai-tester --target wasm32-wasip1 --release
/// Install: copy target/wasm32-wasip1/release/plugin_openai_tester.wasm
///          into plugins/bin/ as plugin-openai-tester.wasm
/// Invoke:  POST /plugins/exec  { "name": "plugin-openai-tester", "input": { ... } }

use extism_pdk::*;
use serde::{Deserialize, Serialize};

#[derive(Deserialize)]
struct Input {
    api_key: String,
    #[serde(default = "default_base_url")]
    base_url: String,
}

fn default_base_url() -> String {
    "https://api.openai.com/v1".to_string()
}

#[derive(Serialize)]
#[serde(untagged)]
enum Output {
    Success { ok: bool, valid: bool, models: Vec<String> },
    Invalid { ok: bool, valid: bool, error: String },
    Err { ok: bool, error: String },
}

/// OpenAI /models response shapes (only the fields we care about).
#[derive(Deserialize)]
struct ModelsResponse {
    data: Vec<ModelObject>,
}

#[derive(Deserialize)]
struct ModelObject {
    id: String,
}

#[plugin_fn]
pub fn execute(raw: String) -> FnResult<String> {
    let input: Input = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            return Ok(serde_json::to_string(&Output::Err {
                ok: false,
                error: format!("invalid input JSON: {}", e),
            })
            .unwrap());
        }
    };

    let base = input.base_url.trim_end_matches('/');
    let url = format!("{}/models", base);

    let req = HttpRequest::new(&url)
        .with_method("GET")
        .with_header("Authorization", &format!("Bearer {}", input.api_key))
        .with_header("User-Agent", "rustboard/plugin-openai-tester");

    let resp = match http::request::<()>(&req, None) {
        Ok(r) => r,
        Err(e) => {
            return Ok(serde_json::to_string(&Output::Invalid {
                ok: true,
                valid: false,
                error: format!("network error: {}", e),
            })
            .unwrap());
        }
    };

    let status = resp.status_code();
    let body = resp.body();

    if status == 401 || status == 403 {
        let body_str = String::from_utf8_lossy(&body).to_string();
        let msg = extract_openai_error(&body_str).unwrap_or_else(|| format!("HTTP {}", status));
        return Ok(serde_json::to_string(&Output::Invalid {
            ok: true,
            valid: false,
            error: msg,
        })
        .unwrap());
    }

    if !(200..300).contains(&(status as i32)) {
        let body_str = String::from_utf8_lossy(&body).to_string();
        let msg = extract_openai_error(&body_str).unwrap_or_else(|| format!("HTTP {}", status));
        return Ok(serde_json::to_string(&Output::Err { ok: false, error: msg }).unwrap());
    }

    let models = serde_json::from_slice::<ModelsResponse>(&body)
        .map(|m| {
            let mut ids: Vec<String> = m.data.into_iter().map(|o| o.id).collect();
            ids.sort();
            ids
        })
        .unwrap_or_default();

    Ok(serde_json::to_string(&Output::Success {
        ok: true,
        valid: true,
        models,
    })
    .unwrap())
}

fn extract_openai_error(body: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    v.get("error")?.get("message")?.as_str().map(|s| s.to_string())
}
