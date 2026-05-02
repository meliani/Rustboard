use anyhow::{Context, Result};
use std::path::Path;
use std::fs;

/// List all `.wasm` plugins in `dir`, returning their stem names (no extension).
pub fn list_plugins_in(dir: &Path) -> Result<Vec<String>> {
    let mut res = Vec::new();
    if !dir.exists() {
        return Ok(res);
    }
    for entry in fs::read_dir(dir).context("reading plugin directory")? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|e| e.to_str()) == Some("wasm") {
            if let Some(stem) = path.file_stem().and_then(|n| n.to_str()) {
                res.push(stem.to_string());
            }
        }
    }
    Ok(res)
}

/// Load and execute a WASM plugin via Extism.
///
/// `path` may omit the `.wasm` extension — it will be resolved automatically.
/// The plugin must export a function named `execute` that takes a JSON string
/// and returns a JSON string (the same contract as the old stdin/stdout protocol).
pub async fn exec_plugin(path: &Path, input: &str) -> Result<String> {
    let resolved = if path.exists() {
        path.to_path_buf()
    } else {
        let with_ext = path.with_extension("wasm");
        if with_ext.exists() {
            with_ext
        } else {
            anyhow::bail!("plugin not found: {}", path.display());
        }
    };

    let input = input.to_string();
    tokio::task::spawn_blocking(move || {
        let wasm = extism::Wasm::file(&resolved);
        // Allow network access for plugins that make outbound HTTP calls.
        // The plugin binary itself is sandboxed — it cannot access the filesystem
        // or spawn processes unless explicitly granted here.
        let manifest = extism::Manifest::new([wasm]).with_allowed_host("*");
        let mut plugin = extism::Plugin::new(&manifest, [], true)
            .context("loading WASM plugin")?;
        let result: String = plugin
            .call("execute", input.as_str())
            .context("executing plugin function")?;
        Ok(result)
    })
    .await
    .context("plugin task panicked")?
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;
    use std::fs::File;
    use std::io::Write;

    #[test]
    fn list_plugins_detects_wasm() {
        let td = tempdir().expect("tempdir");
        let file_path = td.path().join("dummy_plugin.wasm");
        let mut f = File::create(&file_path).expect("create");
        writeln!(f, "").unwrap();

        let list = list_plugins_in(td.path()).expect("list");
        assert!(list.iter().any(|n| n == "dummy_plugin"));
    }

    #[test]
    fn list_plugins_ignores_non_wasm() {
        let td = tempdir().expect("tempdir");
        let file_path = td.path().join("not_a_plugin.exe");
        File::create(&file_path).expect("create");

        let list = list_plugins_in(td.path()).expect("list");
        assert!(list.is_empty());
    }

    #[tokio::test]
    async fn exec_plugin_missing_path_errors() {
        let p = std::path::Path::new("does-not-exist-xyz");
        let res = exec_plugin(p, "{}").await;
        assert!(res.is_err());
    }
}

