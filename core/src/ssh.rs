use anyhow::{Context, Result};
use tokio::process::Command;

pub async fn run_command(ssh_user: Option<&str>, host: &str, cmd: &str) -> Result<String> {
    let target = if let Some(user) = ssh_user { format!("{}@{}", user, host) } else { host.to_string() };
    // Use the system `ssh` binary for simplicity and portability inside WSL/Linux.
    let output = Command::new("ssh")
        .arg("-o")
        .arg("BatchMode=yes")
        .arg(target)
        .arg(cmd)
        .output()
        .await
        .context("failed to spawn ssh")?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let stderr = String::from_utf8_lossy(&output.stderr);
        Err(anyhow::anyhow!("ssh failed: {}", stderr))
    }
}

pub async fn tail_file(ssh_user: Option<&str>, host: &str, path: &str, lines: usize) -> Result<String> {
    let cmd = format!("tail -n {} {}", lines, path);
    run_command(ssh_user, host, &cmd).await
}
