use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Service {
    pub id: String,
    pub name: String,
    pub host: String,
    #[serde(default)]
    pub port: Option<u16>,
    #[serde(default)]
    pub ssh_user: Option<String>,
    #[serde(default)]
    pub start_cmd: Option<String>,
    #[serde(default)]
    pub stop_cmd: Option<String>,
    #[serde(default)]
    pub restart_cmd: Option<String>,
    #[serde(default)]
    pub log_path: Option<String>,
    #[serde(default)]
    pub log_cmd: Option<String>,
    #[serde(default)]
    pub health_cmd: Option<String>,
    #[serde(default)]
    pub health_path: Option<String>,
    #[serde(default)]
    pub dependencies: Vec<String>,
    #[serde(default = "default_status")]
    pub status: String,

    // --- Grouping & metadata ---
    /// Free-form tags for grouping (e.g. ["web", "nginx", "proxy"])
    #[serde(default)]
    pub tags: Vec<String>,
    /// Logical stacks / project names — a service can belong to many (e.g. ["glao", "monitoring"])
    #[serde(default)]
    pub stacks: Vec<String>,

    // --- Container details (optional, informational) ---
    #[serde(default)]
    pub container_name: Option<String>,
    #[serde(default)]
    pub container_id: Option<String>,
    #[serde(default)]
    pub image: Option<String>,

    // --- Discovery marker ---
    /// true = this service was auto-discovered via Docker, not from YAML
    #[serde(default)]
    pub discovered: bool,
}

fn default_status() -> String {
    "unknown".to_string()
}
