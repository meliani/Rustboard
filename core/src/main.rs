use std::sync::Arc;
use std::path::PathBuf;
use std::convert::Infallible;
use axum::{routing::{get, post}, Router, Json, extract::{Extension, Query, Path}};
use axum::response::sse::{Sse, Event};
use axum::response::Html;
use axum::extract::ws::{WebSocketUpgrade, Message, WebSocket};
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use futures::{SinkExt, StreamExt};
use serde_json::Value;
use serde_json::json;
use tokio::sync::RwLock;
mod service;
mod ssh;
mod health;
mod config;
mod plugin;
mod discover;
use service::Service;

// ── Job tracking ──────────────────────────────────────────────────────────────

/// A background execution job — created by `POST /services/exec`.
#[derive(Debug, Clone, serde::Serialize)]
struct Job {
    id:           String,
    service_id:   String,
    cmd:          String,
    /// `"running"` | `"done"` | `"failed"`
    state:        String,
    /// Accumulated output lines (capped at 10 000 to bound memory).
    output:       Vec<String>,
    exit_code:    Option<i32>,
    started_at:   u64,   // ms since Unix epoch
    finished_at:  Option<u64>,
}

/// Returns a unique job ID based on wall-clock time + a monotonic counter.
fn gen_job_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    static CTR: AtomicU64 = AtomicU64::new(0);
    let ts = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    let n = CTR.fetch_add(1, Ordering::Relaxed);
    format!("{:013x}{:03x}", ts, n & 0xfff)
}

fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

// ─────────────────────────────────────────────────────────────────────────────

struct AppState {
    services:    RwLock<Vec<Service>>,
    preferences: RwLock<config::Preferences>,
    broadcaster: broadcast::Sender<String>,
    jobs:        RwLock<std::collections::HashMap<String, Job>>,
}

type SharedState = Arc<AppState>;

async fn list_services(Extension(state): Extension<SharedState>) -> Json<serde_json::Value> {
    let services = state.services.read().await;
    Json(json!({ "services": &*services }))
}

// Perform a service command without holding the write lock during the SSH call.
async fn perform_service_cmd(state: SharedState, id: String, action: String) -> serde_json::Value {
    // collect necessary data and update status under lock
    let (host, ssh_user, cmd_to_run) = {
        let mut services = state.services.write().await;
        if let Some(s) = services.iter_mut().find(|s| s.id == id) {
            let maybe_cmd = match action.as_str() {
                "start" => s.start_cmd.clone(),
                "stop" => s.stop_cmd.clone(),
                "restart" => s.restart_cmd.clone(),
                other => Some(other.to_string()),
            };
            if maybe_cmd.is_none() {
                return json!({"ok": false, "error": "no command configured"});
            }
            // optimistic status update
            s.status = action.clone();
            let s_clone = s.clone();
            let _ = state.broadcaster.send(json!({"type": "service_update", "service": s_clone}).to_string());
            (s.host.clone(), s.ssh_user.clone(), maybe_cmd.unwrap())
        } else {
            return json!({"ok": false, "error": "service not found"});
        }
    };

    // run SSH outside the lock
    match ssh::run_command(ssh_user.as_deref(), &host, &cmd_to_run).await {
        Ok(out) => {
            let mut services = state.services.write().await;
            if let Some(s) = services.iter_mut().find(|s| s.id == id) {
                s.status = action.clone();
                let s_clone = s.clone();
                let _ = state.broadcaster.send(json!({"type": "service_update", "service": s_clone}).to_string());
            }
            json!({"ok": true, "output": out})
        }
        Err(e) => {
            let mut services = state.services.write().await;
            if let Some(s) = services.iter_mut().find(|s| s.id == id) {
                s.status = "error".to_string();
                let s_clone = s.clone();
                let _ = state.broadcaster.send(json!({"type": "service_update", "service": s_clone}).to_string());
            }
            json!({"ok": false, "error": format!("{}", e)})
        }
    }
}

async fn handle_ws(socket: WebSocket, state: SharedState) {
    let (mut sender, mut receiver) = socket.split();
    // subscribe to broadcast channel
    let mut rx = state.broadcaster.subscribe();

    // forward broadcasts to this websocket
    let send_task = tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(msg) => {
                    if sender.send(Message::Text(msg)).await.is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    // read messages from client
    while let Some(Ok(msg)) = receiver.next().await {
        match msg {
            Message::Text(t) => {
                // expect JSON commands: {type: "cmd", id: "...", cmd: "start"}
                if let Ok(v) = serde_json::from_str::<Value>(&t) {
                    if v.get("type").and_then(|x| x.as_str()) == Some("cmd") {
                        if let (Some(id), Some(cmd)) = (v.get("id").and_then(|x| x.as_str()), v.get("cmd").and_then(|x| x.as_str())) {
                            let st = state.clone();
                            let id = id.to_string();
                            let cmd = cmd.to_string();
                            // run command in background; updates will be broadcast
                            tokio::spawn(async move { let _ = perform_service_cmd(st, id, cmd).await; });
                        }
                    }
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }

    // stop send task
    send_task.abort();
}

#[derive(serde::Deserialize)]
struct CmdBody { id: String, cmd: String }

#[derive(serde::Deserialize)]
struct LogsReq { id: String, lines: Option<usize> }
async fn health() -> &'static str { "ok" }

// ── Machine-level info endpoints (no shared state needed) ─────────────────────

#[derive(serde::Deserialize)]
struct MachineQuery { host: String, ssh_user: Option<String> }

/// GET /machines/disk?host=HOST&ssh_user=USER
/// Returns disk usage for `/` on the remote host in kilobytes.
async fn machine_disk(Query(q): Query<MachineQuery>) -> Json<serde_json::Value> {
    // `df -k /` prints sizes in 1-KiB blocks; NR==2 is the data row.
    let cmd = "df -k / 2>/dev/null | awk 'NR==2{print $2, $3, $4}'";
    match ssh::run_command(q.ssh_user.as_deref(), &q.host, cmd).await {
        Ok(out) => {
            let parts: Vec<u64> = out
                .trim()
                .split_whitespace()
                .filter_map(|p| p.parse().ok())
                .collect();
            if parts.len() >= 3 {
                Json(json!({"ok": true, "total_kb": parts[0], "used_kb": parts[1], "avail_kb": parts[2]}))
            } else {
                Json(json!({"ok": false, "error": "unexpected df output", "raw": out.trim()}))
            }
        }
        Err(e) => Json(json!({"ok": false, "error": format!("{}", e)})),
    }
}

/// GET /machines/docker?host=HOST&ssh_user=USER
/// Returns `docker system df --format '{{json .}}'` parsed as a JSON array.
async fn machine_docker(Query(q): Query<MachineQuery>) -> Json<serde_json::Value> {
    let cmd = "docker system df --format '{{json .}}' 2>/dev/null";
    match ssh::run_command(q.ssh_user.as_deref(), &q.host, cmd).await {
        Ok(out) => {
            let items: Vec<serde_json::Value> = out
                .lines()
                .filter_map(|l| serde_json::from_str(l.trim()).ok())
                .collect();
            Json(json!({"ok": true, "items": items}))
        }
        Err(e) => Json(json!({"ok": false, "error": format!("{}", e)})),
    }
}

// Note: handlers for commands/logs/reload are defined as closures in `main`
// because using top-level functions with multiple extractors caused Handler
// trait mismatches in some environments. Topology and list endpoints remain simple.

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt::init();

    let config_path = std::env::args().nth(1).unwrap_or_else(|| "config/services.example.yaml".to_string());
    // Load main config + auto-merge any previously-discovered YAML files
    let initial_services = config::load_all_services(&config_path);
    let pref_path = "config/preferences.yaml";
    let initial_prefs = match config::load_preferences_from_file(pref_path) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("Failed to load prefs {}: {}. Using defaults.", pref_path, e);
            config::Preferences { show_tooltips: true, theme: "dark".to_string() }
        }
    };
    // broadcast channel for server-sent events
    let (bcast_tx, _bcast_rx) = broadcast::channel::<String>(64);
    let state: SharedState = Arc::new(AppState { 
        services:    RwLock::new(initial_services), 
        preferences: RwLock::new(initial_prefs),
        broadcaster: bcast_tx.clone(),
        jobs:        RwLock::new(std::collections::HashMap::new()),
    });

    // Background health check worker
    let health_state = state.clone();
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(10));
        loop {
            interval.tick().await;
            let service_ids: Vec<String> = {
                let s = health_state.services.read().await;
                s.iter().map(|s| s.id.clone()).collect()
            };
            
            for id in service_ids {
                let service_clone = {
                    let s = health_state.services.read().await;
                    s.iter().find(|x| x.id == id).cloned()
                };
                
                if let Some(mut s) = service_clone {
                    // Skip checking if it's currently being "updated" via command (optional optimization)
                    let is_healthy = health::check_service(&s).await;
                    let new_status = if is_healthy { "running".to_string() } else { "stopped".to_string() };
                    
                    if s.status != new_status {
                        let mut services = health_state.services.write().await;
                        if let Some(target) = services.iter_mut().find(|x| x.id == id) {
                            target.status = new_status.clone();
                            s.status = new_status;
                            let _ = health_state.broadcaster.send(json!({"type": "service_update", "service": s}).to_string());
                        }
                    }
                }
            }
        }
    });

    // Plugin directory (can be overridden with PLUGIN_DIR env var).
    // When deployed, plugins live in bin/ next to the executable.
    // When running from source (cargo run), fall back to plugins/bin relative to the workspace root.
    let plugin_dir: PathBuf = std::env::var("PLUGIN_DIR").map(PathBuf::from).unwrap_or_else(|_| {
        let exe_adjacent_bin = std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("bin")));
        if let Some(ref d) = exe_adjacent_bin {
            if d.is_dir() {
                return d.clone();
            }
        }
        PathBuf::from("plugins/bin")
    });

    // Create routes as closures that capture `state` to avoid Handler trait issues
    let state_for_cmd = state.clone();
    let cmd_route = post(move |req: axum::http::Request<axum::body::Body>| {
        let state = state_for_cmd.clone();
        async move {
            let bytes = match hyper::body::to_bytes(req.into_body()).await {
                Ok(b) => b,
                Err(_) => return axum::Json(json!({"ok": false, "error": "failed to read body"})),
            };
            let body: CmdBody = match serde_json::from_slice(&bytes) {
                Ok(b) => b,
                Err(_) => return axum::Json(json!({"ok": false, "error": "invalid json"})),
            };
            let res = perform_service_cmd(state, body.id.clone(), body.cmd.clone()).await;
            axum::Json(res)
        }
    });

    let state_for_logs = state.clone();
    let logs_route = post(move |req: axum::http::Request<axum::body::Body>| {
        let state = state_for_logs.clone();
        async move {
            let bytes = match hyper::body::to_bytes(req.into_body()).await {
                Ok(b) => b,
                Err(_) => return axum::Json(json!({"ok": false, "error": "failed to read body"})),
            };
            let body: LogsReq = match serde_json::from_slice(&bytes) {
                Ok(b) => b,
                Err(_) => return axum::Json(json!({"ok": false, "error": "invalid json"})),
            };
            let services = state.services.read().await;
            if let Some(s) = services.iter().find(|s| s.id == body.id) {
                let lines = body.lines.unwrap_or(200);
                if let Some(cmd) = &s.log_cmd {
                    // Use custom log command. We might want to append `| tail -n <lines>` if not already there or just use it.
                    // For safety, we wrap the command and tail the output.
                    let full_cmd = format!("{} 2>&1 | tail -n {}", cmd, lines);
                    match ssh::run_command(s.ssh_user.as_deref(), &s.host, &full_cmd).await {
                        Ok(out) => return axum::Json(json!({"ok": true, "logs": out})),
                        Err(e) => return axum::Json(json!({"ok": false, "error": format!("{}", e)})),
                    }
                } else if let Some(path) = &s.log_path {
                    match ssh::tail_file(s.ssh_user.as_deref(), &s.host, path, lines).await {
                        Ok(out) => return axum::Json(json!({"ok": true, "logs": out})),
                        Err(e) => return axum::Json(json!({"ok": false, "error": format!("{}", e)})),
                    }
                } else {
                    return axum::Json(json!({"ok": false, "error": "no logs configured"}));
                }
            }
            axum::Json(json!({"ok": false, "error": "service not found"}))
        }
    });

    let state_for_reload = state.clone();
    let reload_route = post(move |_req: axum::http::Request<axum::body::Body>| {
        let state = state_for_reload.clone();
        async move {
            let config_path = std::env::args().nth(1).unwrap_or_else(|| "config/services.example.yaml".to_string());
            let mut ok = true;
            let mut error = String::new();
            
            // Reload services (main config + discovered files)
            {
                let new = config::load_all_services(&config_path);
                let mut s = state.services.write().await;
                *s = new;
            }
            
            // Reload preferences
            let pref_path = "config/preferences.yaml";
            match config::load_preferences_from_file(pref_path) {
                Ok(new) => {
                    let mut p = state.preferences.write().await;
                    *p = new;
                }
                Err(e) => { 
                    if ok { ok = false; error = format!("prefs: {}", e); }
                    else { error = format!("{}; prefs: {}", error, e); }
                }
            }

            if ok {
                let services = state.services.read().await;
                // broadcast full state
                let snapshot = json!({"type": "full_state", "services": &*services}).to_string();
                let _ = state.broadcaster.send(snapshot);
                return axum::Json(json!({"ok": true}));
            }
            axum::Json(json!({"ok": false, "error": error}))
        }
    });

    let state_for_prefs = state.clone();
    let prefs_route = get(move || {
        let state = state_for_prefs.clone();
        async move {
            let p = state.preferences.read().await;
            axum::Json(json!(&*p))
        }
    });

    let state_for_topo = state.clone();
    let topo_route = get(move || {
        let state = state_for_topo.clone();
        async move {
            let services = state.services.read().await;
            let nodes: Vec<_> = services.iter().map(|s| s.id.clone()).collect();
            let edges: Vec<_> = services.iter().flat_map(|s| {
                s.dependencies.iter().map(move |d| json!({"from": s.id, "to": d}))
            }).collect();
            axum::Json(json!({"nodes": nodes, "edges": edges}))
        }
    });

    // SSE events endpoint
    let events_state = state.clone();
    let events_route = get(move || {
        let state = events_state.clone();
        async move {
            // initial snapshot
            let init = {
                let services = state.services.read().await;
                serde_json::to_string(&json!({"type": "full_state", "services": &*services})).unwrap_or_else(|_| "{}".to_string())
            };

            // subscriber stream
            let rx = state.broadcaster.subscribe();
            let bs = BroadcastStream::new(rx).filter_map(|res| async move {
                match res {
                    Ok(s) => Some(Ok::<axum::response::sse::Event, Infallible>(Event::default().data(s))),
                    Err(_) => None,
                }
            });

            let init_stream = tokio_stream::once(Ok(Event::default().data(init)));
            let stream = init_stream.chain(bs);
            Sse::new(stream)
        }
    });

    // WebSocket endpoint (bidirectional)
    let ws_state = state.clone();
    let ws_route = get(move |ws: WebSocketUpgrade| {
        let state = ws_state.clone();
        async move { ws.on_upgrade(move |socket| handle_ws(socket, state)) }
    });

    // Plugin routes: list available plugins, execute a plugin
    let plugin_dir_for_list = plugin_dir.clone();
    let plugins_route = get(move || {
        let plugin_dir = plugin_dir_for_list.clone();
        async move {
            match plugin::list_plugins_in(&plugin_dir) {
                Ok(list) => axum::Json(json!({"ok": true, "plugins": list})),
                Err(e) => axum::Json(json!({"ok": false, "error": format!("{}", e)})),
            }
        }
    });

    #[derive(serde::Deserialize)]
    struct QuickExecReq { id: String, quick: String }
    let state_for_quick = state.clone();
    let quick_exec_route = post(move |req: axum::http::Request<axum::body::Body>| {
        let state = state_for_quick.clone();
        async move {
            let bytes = match hyper::body::to_bytes(req.into_body()).await {
                Ok(b) => b,
                Err(_) => return axum::Json(json!({"ok": false, "error": "failed to read body"})),
            };
            let body: QuickExecReq = match serde_json::from_slice(&bytes) {
                Ok(b) => b,
                Err(_) => return axum::Json(json!({"ok": false, "error": "invalid json — expected {id, quick}"})),
            };

            // Resolve service and quick command by cloning required fields while holding the lock
            let (ssh_user_opt, host_clone, cmd_to_run) = {
                let services = state.services.read().await;
                if let Some(s) = services.iter().find(|s| s.id == body.id) {
                    if let Some(qc) = s.quick_commands.iter().find(|q| q.name == body.quick) {
                        let in_container = qc.in_container.unwrap_or(true);
                        let cmd = if in_container {
                            if let Some(cname) = &s.container_name {
                                format!("docker exec -it {} {}", cname, qc.cmd)
                            } else {
                                qc.cmd.clone()
                            }
                        } else {
                            qc.cmd.clone()
                        };
                        (s.ssh_user.clone(), s.host.clone(), cmd)
                    } else {
                        return axum::Json(json!({"ok": false, "error": "quick command not found"}));
                    }
                } else {
                    return axum::Json(json!({"ok": false, "error": "service not found"}));
                }
            };

            // Run command via SSH (lock released)
            match ssh::run_command(ssh_user_opt.as_deref(), &host_clone, &cmd_to_run).await {
                Ok(out) => return axum::Json(json!({"ok": true, "output": out})),
                Err(e) => return axum::Json(json!({"ok": false, "error": format!("{}", e)})),
            }
        }
    });

    #[derive(serde::Deserialize)]
    struct PluginExec { name: String, input: Option<serde_json::Value> }
    let plugin_dir_for_exec = plugin_dir.clone();
    let plugins_exec_route = post(move |req: axum::http::Request<axum::body::Body>| {
        let plugin_dir = plugin_dir_for_exec.clone();
        async move {
            let bytes = match hyper::body::to_bytes(req.into_body()).await {
                Ok(b) => b,
                Err(_) => return axum::Json(json!({"ok": false, "error": "failed to read body"})),
            };
            let body: PluginExec = match serde_json::from_slice(&bytes) {
                Ok(b) => b,
                Err(_) => return axum::Json(json!({"ok": false, "error": "invalid json"})),
            };
            // Reject names with path separators to prevent directory traversal.
            if body.name.contains('/') || body.name.contains('\\') || body.name.contains("..") {
                return axum::Json(json!({"ok": false, "error": "invalid plugin name"}));
            }
            // Resolve: try name as-is, then with .wasm extension (exec_plugin also does this,
            // but we check early to return a cleaner error message).
            let plugin_path = {
                let base = plugin_dir.join(&body.name);
                if base.exists() {
                    base
                } else {
                    plugin_dir.join(format!("{}.wasm", body.name))
                }
            };
            let input_str = body.input.map(|v| v.to_string()).unwrap_or_default();
            match plugin::exec_plugin(&plugin_path, &input_str).await {
                Ok(out) => axum::Json(json!({"ok": true, "output": out})),
                Err(e) => axum::Json(json!({"ok": false, "error": format!("{}", e)})),
            }
        }
    });

    // --- Discovery routes ---
    #[derive(serde::Deserialize)]
    struct DiscoverReq { host: String, ssh_user: Option<String> }

    let state_for_discover = state.clone();
    let discover_route = post(move |req: axum::http::Request<axum::body::Body>| {
        let state = state_for_discover.clone();
        async move {
            let bytes = match hyper::body::to_bytes(req.into_body()).await {
                Ok(b) => b,
                Err(_) => return axum::Json(json!({"ok": false, "error": "failed to read body"})),
            };
            let body: DiscoverReq = match serde_json::from_slice(&bytes) {
                Ok(b) => b,
                Err(_) => return axum::Json(json!({"ok": false, "error": "invalid json — expected {host, ssh_user}"})),
            };

            // Collect existing service ids so we don't duplicate
            let existing_ids: std::collections::HashSet<String> = {
                let s = state.services.read().await;
                s.iter().map(|x| x.id.clone()).collect()
            };

            match discover::discover_docker_services(
                &body.host,
                body.ssh_user.as_deref(),
                &existing_ids,
            ).await {
                Ok(new_services) => {
                    let count = new_services.len();
                    // Merge into state
                    {
                        let mut s = state.services.write().await;
                        s.extend(new_services.clone());
                    }
                    // Broadcast updated full state
                    {
                        let s = state.services.read().await;
                        let snapshot = json!({"type": "full_state", "services": &*s}).to_string();
                        let _ = state.broadcaster.send(snapshot);
                    }
                    // Persist all discovered services for this host to config/discovered/<host>.yaml
                    {
                        let s = state.services.read().await;
                        let host_services: Vec<_> = s
                            .iter()
                            .filter(|svc| svc.host == body.host && svc.discovered)
                            .cloned()
                            .collect();
                        config::save_discovered_services(&body.host, &host_services);
                    }
                    axum::Json(json!({"ok": true, "discovered": count, "services": new_services}))
                }
                Err(e) => axum::Json(json!({"ok": false, "error": format!("{}", e)})),
            }
        }
    });

    let state_for_hosts = state.clone();
    let discover_hosts_route = get(move || {
        let state = state_for_hosts.clone();
        async move {
            let s = state.services.read().await;
            // Unique hosts with count
            let mut map: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
            for svc in s.iter() {
                *map.entry(svc.host.clone()).or_insert(0) += 1;
            }
            let hosts: Vec<_> = map.into_iter().map(|(h, c)| json!({"host": h, "service_count": c})).collect();
            axum::Json(json!({"ok": true, "hosts": hosts}))
        }
    });

    // POST /machines/forget — remove all services for a host and delete its discovered file
    #[derive(serde::Deserialize)]
    struct ForgetReq { host: String }

    let state_for_forget = state.clone();
    let forget_route = post(move |req: axum::http::Request<axum::body::Body>| {
        let state = state_for_forget.clone();
        async move {
            let bytes = match hyper::body::to_bytes(req.into_body()).await {
                Ok(b) => b,
                Err(_) => return axum::Json(json!({"ok": false, "error": "failed to read body"})),
            };
            let body: ForgetReq = match serde_json::from_slice(&bytes) {
                Ok(b) => b,
                Err(_) => return axum::Json(json!({"ok": false, "error": "invalid json — expected {host}"})),
            };
            {
                let mut s = state.services.write().await;
                s.retain(|svc| svc.host != body.host);
            }
            // Remove the persisted discovered file so it won't be reloaded
            config::forget_discovered_host(&body.host);
            // Broadcast updated state
            {
                let s = state.services.read().await;
                let snapshot = json!({"type": "full_state", "services": &*s}).to_string();
                let _ = state.broadcaster.send(snapshot);
            }
            axum::Json(json!({"ok": true}))
        }
    });

    // ── POST /services/exec ────────────────────────────────────────────────────
    // Starts a background job that streams command output via the broadcast
    // channel.  Returns {ok, job_id} immediately so the caller can subscribe to
    // `job_output` / `job_done` WebSocket events without blocking.
    #[derive(serde::Deserialize)]
    struct ExecReq { id: String, cmd: String }

    let state_for_exec = state.clone();
    let exec_route = post(move |req: axum::http::Request<axum::body::Body>| {
        let state = state_for_exec.clone();
        async move {
            let bytes = match hyper::body::to_bytes(req.into_body()).await {
                Ok(b) => b,
                Err(_) => return axum::Json(json!({"ok": false, "error": "failed to read body"})),
            };
            let body: ExecReq = match serde_json::from_slice(&bytes) {
                Ok(b) => b,
                Err(_) => return axum::Json(json!({"ok": false, "error": "invalid json — expected {id, cmd}"})),
            };

            // Resolve host / ssh_user while holding a read lock only
            let (host, ssh_user) = {
                let services = state.services.read().await;
                match services.iter().find(|s| s.id == body.id) {
                    Some(s) => (s.host.clone(), s.ssh_user.clone()),
                    None    => return axum::Json(json!({"ok": false, "error": "service not found"})),
                }
            };

            // Create the job record
            let job_id = gen_job_id();
            {
                let mut jobs = state.jobs.write().await;
                jobs.insert(job_id.clone(), Job {
                    id:          job_id.clone(),
                    service_id:  body.id.clone(),
                    cmd:         body.cmd.clone(),
                    state:       "running".to_string(),
                    output:      Vec::new(),
                    exit_code:   None,
                    started_at:  now_millis(),
                    finished_at: None,
                });
            }

            // Spawn background worker: stream output → job store + broadcast
            let state_bg = state.clone();
            let jid      = job_id.clone();
            let cmd      = body.cmd.clone();
            tokio::spawn(async move {
                let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(1024);

                // Collector task: receives lines, appends to job, broadcasts them
                let state2 = state_bg.clone();
                let jid2   = jid.clone();
                let collector = tokio::spawn(async move {
                    while let Some(line) = rx.recv().await {
                        {
                            let mut jobs = state2.jobs.write().await;
                            if let Some(j) = jobs.get_mut(&jid2) {
                                j.output.push(line.clone());
                                // Bound memory: keep at most 10 000 lines
                                if j.output.len() > 10_000 {
                                    j.output.remove(0);
                                }
                            }
                        }
                        let _ = state2.broadcaster.send(
                            json!({"type": "job_output", "job_id": &jid2, "line": line}).to_string()
                        );
                    }
                });

                // Run the SSH command; each line is forwarded via `tx`
                let exit_code = ssh::run_command_streaming(
                    ssh_user.as_deref(), &host, &cmd, tx,
                ).await.unwrap_or(-1);

                // Drain the collector before marking the job finished
                let _ = collector.await;

                // Finalise job state
                {
                    let mut jobs = state_bg.jobs.write().await;
                    if let Some(j) = jobs.get_mut(&jid) {
                        j.state       = if exit_code == 0 { "done".to_string() }
                                        else              { "failed".to_string() };
                        j.exit_code   = Some(exit_code);
                        j.finished_at = Some(now_millis());
                    }
                }

                // Broadcast completion event so the UI knows the command finished
                let _ = state_bg.broadcaster.send(
                    json!({"type": "job_done", "job_id": &jid, "exit_code": exit_code}).to_string()
                );
            });

            axum::Json(json!({"ok": true, "job_id": job_id}))
        }
    });

    // ── GET /jobs/:id ──────────────────────────────────────────────────────────
    // Fetch a full job snapshot — useful for late joiners that missed WS events.
    let state_for_jobs = state.clone();
    let job_get_route = get(move |Path(job_id): Path<String>| {
        let state = state_for_jobs.clone();
        async move {
            let jobs = state.jobs.read().await;
            match jobs.get(&job_id) {
                Some(j) => axum::Json(json!({"ok": true,  "job": j})),
                None    => axum::Json(json!({"ok": false, "error": "job not found"})),
            }
        }
    });

    let app = Router::new()
        .route("/", get(|| async { Html(include_str!("../web/index.html")) }))
        .route("/services", get(list_services))
        .route("/events", events_route)
        .route("/ws", ws_route)
        .route("/services/cmd", cmd_route)
        .route("/services/quick", quick_exec_route)
        .route("/services/logs", logs_route)
        .route("/services/exec", exec_route)
        .route("/jobs/:id", job_get_route)
        .route("/config/reload", reload_route)
        .route("/preferences", prefs_route)
        .route("/plugins", plugins_route)
        .route("/plugins/exec", plugins_exec_route)
        .route("/topology", topo_route)
        .route("/discover", discover_route)
        .route("/discover/hosts", discover_hosts_route)
        .route("/machines/disk", get(machine_disk))
        .route("/machines/docker", get(machine_docker))
        .route("/machines/forget", forget_route)
        .route("/health", get(health))
        .layer(Extension(state));
    let addr = std::net::SocketAddr::from(([127,0,0,1], 8080));
    tracing::info!("Rustboard listening on {}", addr);

    // Open the dashboard in the default browser shortly after binding.
    // Runs in background so it doesn't block the server from starting.
    tokio::spawn(async {
        // Give the server a moment to finish binding before opening the browser.
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        let url = "http://127.0.0.1:8080";
        #[cfg(target_os = "windows")]
        let _ = tokio::process::Command::new("cmd").args(["/c", "start", url]).spawn();
        #[cfg(target_os = "macos")]
        let _ = tokio::process::Command::new("open").arg(url).spawn();
        #[cfg(not(any(target_os = "windows", target_os = "macos")))]
        let _ = tokio::process::Command::new("xdg-open").arg(url).spawn();
    });

    axum::Server::bind(&addr).serve(app.into_make_service()).await?;
    Ok(())
}
