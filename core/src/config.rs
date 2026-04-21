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
