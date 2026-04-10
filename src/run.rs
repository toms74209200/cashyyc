use crate::cli;
use crate::devcontainer;
use crate::docker;
use anyhow::{Result, anyhow};

pub fn run(args: Vec<String>) -> Result<()> {
    match cli::parser::parse_args(&args) {
        cli::parser::Command::Shell { name: _ } => {
            let cwd = std::env::current_dir()?;
            let config_path = cwd.join(".devcontainer").join("devcontainer.json");
            let content = std::fs::read_to_string(&config_path).map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    anyhow!(
                        "Dev container config ({}) not found.",
                        config_path.display()
                    )
                } else {
                    anyhow!("Dev container config ({}): {e}", config_path.display())
                }
            })?;
            let _config = devcontainer::parse_config(&content).ok_or_else(|| {
                anyhow!(
                    "Failed to parse dev container config ({}).",
                    config_path.display()
                )
            })?;
            let output = std::process::Command::new("docker")
                .args([
                    "ps",
                    "--filter",
                    &format!("label=devcontainer.local_folder={}", cwd.display()),
                    "--format",
                    "{{.ID}}",
                ])
                .output()
                .map_err(|e| anyhow!("Failed to run docker: {e}"))?;
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(anyhow!(
                    "`docker ps` failed with status {}: {}",
                    output.status,
                    stderr.trim()
                ));
            }
            let stdout = String::from_utf8_lossy(&output.stdout);
            let _container_id = docker::parse_container_id(&stdout);
            Ok(())
        }
        cli::parser::Command::Unknown(msg) => Err(anyhow!(msg)),
    }
}
