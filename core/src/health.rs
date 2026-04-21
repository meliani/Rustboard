use crate::service::Service;
use std::time::Duration;

#[allow(dead_code)]
pub async fn check_service(s: &Service) -> bool {
    // 1. Prefer health_cmd if available (useful for remote Docker/SSH)
    if let Some(cmd) = &s.health_cmd {
        match crate::ssh::run_command(s.ssh_user.as_deref(), &s.host, cmd).await {
            Ok(output) => {
                let low = output.to_lowercase();
                // For Docker, we often want 'healthy' or at least 'Up'
                return low.contains("healthy") || low.contains("up") || (low.contains("running") && !low.contains("stopped"));
            }
            Err(_) => return false,
        }
    }

    // 2. HTTP check if port or health_path available
    let port = s.port.unwrap_or(80);
    let path = s.health_path.as_deref().unwrap_or("/");
    let url_str = format!("http://{}:{}{}", s.host, port, path);
    // ...
    if let Ok(u) = url_str.parse::<reqwest::Url>() {
        match reqwest::Client::builder().timeout(Duration::from_secs(2)).build() {
            Ok(client) => {
                if let Ok(res) = client.get(u).send().await {
                   if res.status().is_success() {
                       return true;
                   }
                }
            }
            Err(_) => {}
        }
    }
    
    // 3. Fallback: simple TCP connection check if HTTP fails
    if let Some(p) = s.port {
        if check_tcp(&s.host, p).await {
            return true;
        }
    }
    false
}

#[allow(dead_code)]
pub async fn check_tcp(host: &str, port: u16) -> bool {
    use tokio::net::TcpStream;
    tokio::time::timeout(std::time::Duration::from_secs(1), TcpStream::connect((host, port))).await.is_ok()
}
