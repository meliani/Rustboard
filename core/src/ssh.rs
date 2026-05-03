use anyhow::{Context, Result};
use tokio::process::Command;

/// SSH options applied to every remote call.
/// - BatchMode=yes             : never prompt for passwords — fail fast on auth issues.
/// - ConnectTimeout=10         : don't hang forever on unreachable hosts.
/// - ServerAliveInterval=15    : send keep-alive probes every 15 s so the kernel
///                               detects a dead connection before the OS TCP timeout.
/// - ServerAliveCountMax=3     : drop after 3 unanswered probes (~45 s of silence).
/// - StrictHostKeyChecking=accept-new : auto-accept new host keys (safe on private infra)
///                               but still reject changed keys to detect MITM.
const SSH_OPTS: &[&str] = &[
    "-o", "BatchMode=yes",
    "-o", "ConnectTimeout=10",
    "-o", "ServerAliveInterval=15",
    "-o", "ServerAliveCountMax=3",
    "-o", "StrictHostKeyChecking=accept-new",
];

fn ssh_target(ssh_user: Option<&str>, host: &str) -> String {
    match ssh_user {
        Some(u) => format!("{}@{}", u, host),
        None    => host.to_string(),
    }
}

/// Run a remote command and collect its full output.
/// Stderr is included (the remote shell merges it into stdout).
pub async fn run_command(ssh_user: Option<&str>, host: &str, cmd: &str) -> Result<String> {
    let target = ssh_target(ssh_user, host);
    let output = Command::new("ssh")
        .args(SSH_OPTS)
        .arg(&target)
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

/// Run a remote command and stream each line of output through `tx`.
/// Both stdout and stderr are captured (stderr is merged server-side with `2>&1`).
///
/// Returns the remote exit code, or -1 when the process was killed / unavailable.
/// Dropping the receiver (`tx`) before the command finishes causes the SSH
/// process to be abandoned (the function returns early with the last known code).
pub async fn run_command_streaming(
    ssh_user: Option<&str>,
    host: &str,
    cmd: &str,
    tx: tokio::sync::mpsc::Sender<String>,
) -> Result<i32> {
    use std::process::Stdio;
    use tokio::io::AsyncBufReadExt as _;

    let target = ssh_target(ssh_user, host);
    // Merge remote stderr into stdout so we have one ordered stream.
    let merged_cmd = format!("{{ {}; }} 2>&1", cmd);

    let mut child = Command::new("ssh")
        .args(SSH_OPTS)
        .arg(&target)
        .arg(&merged_cmd)
        .stdout(Stdio::piped())
        .stderr(Stdio::null())  // local SSH errors go to /dev/null — they arrive on stdout anyway
        .spawn()
        .context("failed to spawn ssh")?;

    let stdout = child.stdout.take().expect("stdout was piped");
    let mut reader = tokio::io::BufReader::new(stdout).lines();

    while let Some(line) = reader.next_line().await? {
        if tx.send(line).await.is_err() {
            // Receiver dropped — stop reading but let the process finish
            break;
        }
    }

    let status = child.wait().await.context("failed to wait for ssh process")?;
    Ok(status.code().unwrap_or(-1))
}

pub async fn tail_file(ssh_user: Option<&str>, host: &str, path: &str, lines: usize) -> Result<String> {
    let cmd = format!("tail -n {} {}", lines, path);
    run_command(ssh_user, host, &cmd).await
}
