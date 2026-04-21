use crate::service::{Service, QuickCommand};
use anyhow::Result;
use serde::Deserialize;

/// Raw output of `docker ps --format '{{json .}}'` — one JSON object per line.
#[derive(Debug, Deserialize)]
#[allow(non_snake_case, dead_code)]
struct DockerPsEntry {
    #[serde(rename = "ID")]
    id: Option<String>,
    #[serde(rename = "Names")]
    names: Option<String>,
    #[serde(rename = "Image")]
    image: Option<String>,
    #[serde(rename = "Ports")]
    ports: Option<String>,
    #[serde(rename = "Status")]
    status: Option<String>,
    #[serde(rename = "Labels")]
    labels: Option<String>,
    #[serde(rename = "State")]
    state: Option<String>,
}

/// Discover all running (and exited) Docker containers on a remote host via SSH.
/// Returns the list of `Service` entries that were found but are NOT already
/// present in `existing_ids`.
pub async fn discover_docker_services(
    host: &str,
    ssh_user: Option<&str>,
    existing_ids: &std::collections::HashSet<String>,
) -> Result<Vec<Service>> {
    // Ask Docker for a JSON line per container (all, including stopped)
    let cmd = "docker ps -a --format '{{json .}}'";
    let raw = crate::ssh::run_command(ssh_user, host, cmd).await?;

    let mut discovered: Vec<Service> = Vec::new();

    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }

        let entry: DockerPsEntry = match serde_json::from_str(line) {
            Ok(e) => e,
            Err(e) => {
                tracing::warn!("discover: failed to parse docker ps line: {} — {:?}", line, e);
                continue;
            }
        };

        let raw_name = entry.names.clone().unwrap_or_default();
        // Docker may return comma-separated names; take the first
        let container_name = raw_name.split(',').next().unwrap_or(&raw_name).trim_start_matches('/').to_string();
        if container_name.is_empty() {
            continue;
        }

        // Use container name as the service id (sanitised)
        let id = sanitise_id(&container_name);

        // Skip if a service with this id already exists
        if existing_ids.contains(&id) {
            continue;
        }

        let image = entry.image.clone().unwrap_or_default();

        // -- Parse stack / tags from Docker labels --
        // Labels string format from docker ps: "key=val,key2=val2"
        let (stacks, mut tags) = parse_labels(entry.labels.as_deref().unwrap_or(""));

        // Infer additional tags from image name
        tags.extend(infer_tags_from_image(&image));

        // -- Parse port --
        let port = parse_first_host_port(entry.ports.as_deref().unwrap_or(""));

        // -- Determine status --
        let status = match entry.state.as_deref().unwrap_or("") {
            "running" => "running".to_string(),
            "exited" | "dead" => "stopped".to_string(),
            other if !other.is_empty() => other.to_string(),
            _ => {
                // Fall back to Status string heuristic
                let st = entry.status.as_deref().unwrap_or("").to_lowercase();
                if st.starts_with("up") { "running".to_string() } else { "stopped".to_string() }
            }
        };

        let short_id = entry.id.clone().unwrap_or_default();
        let short_id = short_id.trim().to_string();

        // Build log and control commands using the container name
        let log_cmd = format!("docker logs {} 2>&1 | tail -n 200", container_name);
        let start_cmd = format!("docker start {}", container_name);
        let stop_cmd = format!("docker stop {}", container_name);
        let restart_cmd = format!("docker restart {}", container_name);
        let health_cmd = format!("docker inspect --format '{{{{.State.Status}}}}' {}", container_name);
        // Attempt to inspect container to guess working dir / mounts
        let mut predicted_app_path: Option<String> = None;
        // Inspect WorkingDir
        let inspect_wd_cmd = format!("docker inspect --format '{{{{json .Config.WorkingDir}}}}' {}", container_name);
        if let Ok(wd_raw) = crate::ssh::run_command(ssh_user, host, &inspect_wd_cmd).await {
            if let Ok(pwd) = serde_json::from_str::<Option<String>>(&wd_raw) {
                if let Some(p) = pwd {
                    if !p.is_empty() {
                        predicted_app_path = Some(p);
                    }
                }
            }
        }

        // Inspect mounts to see if any destination looks like an app folder (e.g. contains "app" or "www")
        if predicted_app_path.is_none() {
            let inspect_mounts_cmd = format!("docker inspect --format '{{{{json .Mounts}}}}' {}", container_name);
            if let Ok(m_raw) = crate::ssh::run_command(ssh_user, host, &inspect_mounts_cmd).await {
                if let Ok(mv) = serde_json::from_str::<serde_json::Value>(&m_raw) {
                    if let Some(arr) = mv.as_array() {
                        for m in arr {
                            if let Some(dest) = m.get("Destination").and_then(|d| d.as_str()) {
                                let ds = dest.to_lowercase();
                                if ds.contains("/app") || ds.contains("/www") || ds.contains("/srv") || ds.contains("/usr/src") {
                                    predicted_app_path = Some(dest.to_string());
                                    break;
                                }
                            }
                        }
                    }
                }
            }
        }

        // Prepare sensible default quick commands
        let mut quicks: Vec<QuickCommand> = Vec::new();
        // Interactive shell: prefer bash, fallback to sh when executed via `sh -lc 'bash -l || sh'`
        quicks.push(QuickCommand { name: "shell".to_string(), cmd: "sh -lc 'bash -l || sh'".to_string(), description: Some("Interactive shell inside container".to_string()), in_container: Some(true) });
        // ls of predicted app path (or root)
        let list_path = predicted_app_path.clone().unwrap_or_else(|| "/".to_string());
        quicks.push(QuickCommand { name: "ls_app".to_string(), cmd: format!("sh -lc 'ls -la {} || ls -la /'", list_path), description: Some("List application folder inside container".to_string()), in_container: Some(true) });

        discovered.push(Service {
            id,
            name: container_name.clone(),
            host: host.to_string(),
            port,
            ssh_user: ssh_user.map(|s| s.to_string()),
            start_cmd: Some(start_cmd),
            stop_cmd: Some(stop_cmd),
            restart_cmd: Some(restart_cmd),
            log_path: None,
            log_cmd: Some(log_cmd),
            health_cmd: Some(health_cmd),
            health_path: None,
            dependencies: vec![],
            status,
            tags,
            stacks,
            container_name: Some(container_name),
            container_id: if short_id.is_empty() { None } else { Some(short_id) },
            image: if image.is_empty() { None } else { Some(image) },
            discovered: true,
            quick_commands: quicks,
            predicted_app_path,
        });
    }

    Ok(discovered)
}

// ---------- helpers ----------

fn sanitise_id(s: &str) -> String {
    s.chars()
        .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '-' })
        .collect::<String>()
        .to_lowercase()
}

/// Parse Docker label string "key=val,key2=val2" into (stacks, tags).
fn parse_labels(labels: &str) -> (Vec<String>, Vec<String>) {
    let mut stacks: Vec<String> = Vec::new();
    let mut tags: Vec<String> = Vec::new();

    for pair in labels.split(',') {
        let mut kv = pair.splitn(2, '=');
        let key = kv.next().unwrap_or("").trim();
        let val = kv.next().unwrap_or("").trim().to_string();

        match key {
            "com.docker.compose.project" if !val.is_empty() => {
                if !stacks.contains(&val) { stacks.push(val.clone()); }
                tags.push(val);
            }
            _ => {}
        }
    }

    (stacks, tags)
}

/// Guess tags from an image name such as "nginx:stable-alpine", "redis:7", etc.
fn infer_tags_from_image(image: &str) -> Vec<String> {
    let base = image.split(':').next().unwrap_or("").split('/').last().unwrap_or("");
    let known = [
        "nginx", "redis", "postgres", "mysql", "mongodb", "rabbitmq",
        "kafka", "zookeeper", "elasticsearch", "kibana", "grafana",
        "prometheus", "traefik", "caddy", "php", "node", "python",
        "rust", "go", "java", "dotnet",
    ];
    let mut found = Vec::new();
    for k in &known {
        if base.to_lowercase().contains(k) {
            found.push(k.to_string());
        }
    }
    found
}

/// Parse the first exposed host port from Docker's Ports field.
/// Format examples: "0.0.0.0:8080->80/tcp", "80/tcp, 443/tcp"
fn parse_first_host_port(ports: &str) -> Option<u16> {
    for segment in ports.split(',') {
        let segment = segment.trim();
        // Pattern: "0.0.0.0:HOST_PORT->CONTAINER_PORT/proto"
        if let Some(arrow_pos) = segment.find("->") {
            let left = &segment[..arrow_pos];
            if let Some(colon_pos) = left.rfind(':') {
                let port_str = &left[colon_pos + 1..];
                if let Ok(p) = port_str.parse::<u16>() {
                    return Some(p);
                }
            }
        }
    }
    None
}
