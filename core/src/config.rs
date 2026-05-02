use crate::service::Service;
use anyhow::Result;
use std::fs;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Preferences {
    pub show_tooltips: bool,
    #[serde(default = "default_theme")]
    pub theme: String,
}

fn default_theme() -> String { "dark".to_string() }

pub fn load_services_from_file(path: &str) -> Result<Vec<Service>> {
    let data = fs::read_to_string(path)?;
    let services: Vec<Service> = serde_yaml::from_str(&data)?;
    Ok(services)
}

pub fn load_preferences_from_file(path: &str) -> Result<Preferences> {
    let data = fs::read_to_string(path)?;
    let prefs: Preferences = serde_yaml::from_str(&data)?;
    Ok(prefs)
}

/// Load services from the main config file, then merge any YAML files found in
/// `config/discovered/`.  Deduplicates by service id so that services already
/// present in the main config are never overwritten by a discovered copy.
pub fn load_all_services(config_path: &str) -> Vec<Service> {
    let mut services = match load_services_from_file(config_path) {
        Ok(s) => s,
        Err(_) => Vec::new(),
    };

    let discovered_dir = "config/discovered";
    if let Ok(entries) = fs::read_dir(discovered_dir) {
        let mut existing_ids: std::collections::HashSet<String> =
            services.iter().map(|s| s.id.clone()).collect();
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().map_or(false, |e| e == "yaml" || e == "yml") {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(svcs) = serde_yaml::from_str::<Vec<Service>>(&content) {
                        for svc in svcs {
                            if !existing_ids.contains(&svc.id) {
                                existing_ids.insert(svc.id.clone());
                                services.push(svc);
                            }
                        }
                    }
                }
            }
        }
    }
    services
}

/// Persist a list of discovered services for a given host to
/// `config/discovered/<safe_host>.yaml`.
pub fn save_discovered_services(host: &str, services: &[Service]) {
    let safe_host: String = host
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '_' })
        .collect();
    let _ = fs::create_dir_all("config/discovered");
    if let Ok(yaml) = serde_yaml::to_string(services) {
        let _ = fs::write(format!("config/discovered/{}.yaml", safe_host), yaml);
    }
}

/// Delete the discovered-services file for a given host.
pub fn forget_discovered_host(host: &str) {
    let safe_host: String = host
        .chars()
        .map(|c| if c.is_alphanumeric() || c == '-' { c } else { '_' })
        .collect();
    let _ = fs::remove_file(format!("config/discovered/{}.yaml", safe_host));
}
