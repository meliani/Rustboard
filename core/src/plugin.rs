use anyhow::{Context, Result};
use std::path::Path;
use std::{fs};
use tokio::process::Command;
use tokio::io::AsyncWriteExt;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

#[cfg(unix)]
fn is_executable(path: &Path) -> bool {
    match path.metadata() {
        Ok(meta) => (meta.permissions().mode() & 0o111) != 0,
        Err(_) => false,
    }
}

#[cfg(windows)]
fn is_executable(path: &Path) -> bool {
    match path.extension().and_then(|s| s.to_str()) {
        Some(ext) => matches!(ext.to_lowercase().as_str(), "exe" | "bat" | "cmd" | "ps1"),
        None => false,
    }
}

pub fn list_plugins_in(dir: &Path) -> Result<Vec<String>> {
    let mut res = Vec::new();
    if !dir.exists() {
        return Ok(res);
    }
    for entry in fs::read_dir(dir).context("reading plugin directory")? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && is_executable(&path) {
            if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                res.push(name.to_string());
            }
        }
    }
    Ok(res)
}

pub async fn exec_plugin(path: &Path, input: &str) -> Result<String> {
    let mut cmd = Command::new(path);
    let mut child = cmd
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .context("spawning plugin process")?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin.write_all(input.as_bytes()).await.context("writing to plugin stdin")?;
    }

    let output = child.wait_with_output().await.context("waiting for plugin output")?;
    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    } else {
        let err = String::from_utf8_lossy(&output.stderr).to_string();
        Err(anyhow::anyhow!("plugin failed: {}", err))
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs::File;
    use std::io::Write;

    #[test]
    fn list_plugins_detects_executable() {
        let td = tempdir().expect("tempdir");
        let mut file_path = td.path().join("dummy_plugin");
        #[cfg(windows)]
        {
            file_path.set_extension("exe");
        }
        let mut f = File::create(&file_path).expect("create");
        writeln!(f, "echo hello").unwrap();
        // Ensure executable bit on unix
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let metadata = std::fs::metadata(&file_path).expect("meta");
            let mut perms = metadata.permissions();
            perms.set_mode(0o755);
            std::fs::set_permissions(&file_path, perms).expect("set perms");
        }

        let list = list_plugins_in(td.path()).expect("list");
        let file_name = file_path.file_name().and_then(|n| n.to_str()).unwrap();
        assert!(list.iter().any(|n| n == file_name));
    }

    #[tokio::test]
    async fn exec_plugin_missing_path_errors() {
        let p = std::path::Path::new("does-not-exist-xyz");
        let res = exec_plugin(p, "{}").await;
        assert!(res.is_err());
    }
}
