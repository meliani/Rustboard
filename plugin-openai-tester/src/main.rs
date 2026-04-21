/// plugin-openai-tester
///
/// Plugin protocol (matches the dashboard plugin contract):
///   stdin  → JSON: { "api_key": "sk-...", "base_url": "https://api.openai.com/v1" }
///            `base_url` is optional; defaults to https://api.openai.com/v1
///            Any OpenAI-compatible endpoint works (Azure, Ollama, Groq, etc.)
///   stdout → JSON: { "ok": true,  "valid": true,  "models": ["gpt-4o", ...] }
///          | JSON: { "ok": true,  "valid": false, "error": "..." }
///          | JSON: { "ok": false, "error": "..." }   ← bad input / network fail
///
/// Install: copy / symlink the compiled binary into the `plugins/` directory.
/// Invoke:  POST /plugins/exec  { "name": "plugin-openai-tester", "input": { ... } }

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

#[tokio::main]
async fn main() {
    let raw = read_stdin();

    let input: Input = match serde_json::from_str(&raw) {
        Ok(v) => v,
        Err(e) => {
            print_output(Output::Err { ok: false, error: format!("invalid input JSON: {}", e) });
            return;
        }
    };

    // Sanitise: strip trailing slash so we can append /models cleanly.
    let base = input.base_url.trim_end_matches('/');
    let url = format!("{}/models", base);

    let client = match reqwest::Client::builder()
        .user_agent("rustboard/plugin-openai-tester")
        .build()
    {
        Ok(c) => c,
        Err(e) => {
            print_output(Output::Err { ok: false, error: format!("failed to build HTTP client: {}", e) });
            return;
        }
    };

    let resp = client
        .get(&url)
        .header("Authorization", format!("Bearer {}", input.api_key))
        .send()
        .await;

    match resp {
        Err(e) => {
            print_output(Output::Invalid {
                ok: true,
                valid: false,
                error: format!("network error: {}", e),
            });
        }
        Ok(r) => {
            let status = r.status();
            if status == reqwest::StatusCode::UNAUTHORIZED || status == reqwest::StatusCode::FORBIDDEN {
                let body = r.text().await.unwrap_or_default();
                let msg = extract_openai_error(&body).unwrap_or_else(|| format!("HTTP {}", status));
                print_output(Output::Invalid { ok: true, valid: false, error: msg });
                return;
            }
            if !status.is_success() {
                let body = r.text().await.unwrap_or_default();
                let msg = extract_openai_error(&body).unwrap_or_else(|| format!("HTTP {}", status));
                print_output(Output::Err { ok: false, error: msg });
                return;
            }
            // Parse model list — if the endpoint doesn't return the standard shape just
            // report valid=true with an empty model list rather than failing.
            let models = r
                .json::<ModelsResponse>()
                .await
                .map(|m| {
                    let mut ids: Vec<String> = m.data.into_iter().map(|o| o.id).collect();
                    ids.sort();
                    ids
                })
                .unwrap_or_default();

            print_output(Output::Success { ok: true, valid: true, models });
        }
    }
}

fn read_stdin() -> String {
    use std::io::Read;
    let mut buf = String::new();
    std::io::stdin().read_to_string(&mut buf).unwrap_or(0);
    buf.trim().to_string()
}

fn print_output(out: Output) {
    println!("{}", serde_json::to_string(&out).unwrap_or_else(|_| r#"{"ok":false,"error":"serialization error"}"#.to_string()));
}

/// Try to extract the `error.message` field from an OpenAI-style error body.
fn extract_openai_error(body: &str) -> Option<String> {
    let v: serde_json::Value = serde_json::from_str(body).ok()?;
    v.get("error")?.get("message")?.as_str().map(|s| s.to_string())
}
