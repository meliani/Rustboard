use crate::service::Service;
use anyhow::Result;
use std::fs;

pub fn load_services_from_file(path: &str) -> Result<Vec<Service>> {
    let data = fs::read_to_string(path)?;
    let services: Vec<Service> = serde_yaml::from_str(&data)?;
    Ok(services)
}
