use crate::cli;
use crate::devcontainer;
use crate::docker;
use anyhow::{Result, anyhow};

pub fn run(args: Vec<String>) -> Result<()> {
    match cli::parse_args(&args) {
        cli::Command::Shell { name: _ } => {
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
            let config = devcontainer::parse_config(&content).ok_or_else(|| {
                anyhow!(
                    "Failed to parse dev container config ({}).",
                    config_path.display()
                )
            })?;
            let output = std::process::Command::new("docker")
                .args([
                    "ps",
                    "-a",
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
            let mut container_id = docker::parse_container_id(&stdout);
            if container_id.is_none() {
                match config {
                    devcontainer::DevcontainerConfig::Image(ref image_config) => {
                        let status = std::process::Command::new("docker")
                            .args(["pull", &image_config.image])
                            .status()
                            .map_err(|e| anyhow!("Failed to run docker: {e}"))?;
                        if !status.success() {
                            return Err(anyhow!("`docker pull` failed"));
                        }
                        let container_workspace_folder = image_config
                            .common
                            .workspace_folder
                            .clone()
                            .unwrap_or_else(|| {
                                format!(
                                    "/workspaces/{}",
                                    cwd.file_name().unwrap_or_default().to_string_lossy()
                                )
                            });
                        let workspace_mount = image_config.workspace_mount.as_deref().map(|m| {
                            devcontainer::expand_variables(m, &cwd, &container_workspace_folder)
                        });
                        let mut run_args = devcontainer::container_run_options(
                            &image_config.common,
                            image_config.run_args.as_deref(),
                            workspace_mount.as_deref(),
                            &cwd,
                        );
                        run_args.push(image_config.image.clone());
                        if image_config.common.override_command != Some(false) {
                            run_args.extend(["sleep".to_string(), "infinity".to_string()]);
                        }
                        let output = std::process::Command::new("docker")
                            .arg("run")
                            .args(&run_args)
                            .output()
                            .map_err(|e| anyhow!("Failed to run docker: {e}"))?;
                        if !output.status.success() {
                            let stderr = String::from_utf8_lossy(&output.stderr);
                            return Err(anyhow!("`docker run` failed: {}", stderr.trim()));
                        }
                        container_id =
                            docker::parse_container_id(&String::from_utf8_lossy(&output.stdout));
                    }
                    devcontainer::DevcontainerConfig::Dockerfile(_)
                    | devcontainer::DevcontainerConfig::DockerfileBuild(_) => {
                        return Err(anyhow!("DockerfileConfig: not yet implemented"));
                    }
                    devcontainer::DevcontainerConfig::DockerCompose(_) => {
                        return Err(anyhow!("DockerComposeConfig: not yet implemented"));
                    }
                }
            }
            let id = container_id.ok_or_else(|| anyhow!("Failed to get container ID"))?;
            let remote_user = match &config {
                devcontainer::DevcontainerConfig::Image(c) => c.common.remote_user.as_deref(),
                devcontainer::DevcontainerConfig::Dockerfile(c) => c.common.remote_user.as_deref(),
                devcontainer::DevcontainerConfig::DockerfileBuild(c) => {
                    c.common.remote_user.as_deref()
                }
                devcontainer::DevcontainerConfig::DockerCompose(c) => {
                    c.common.remote_user.as_deref()
                }
            };
            let mut exec_args = vec!["exec".to_string(), "-it".to_string()];
            if let Some(user) = remote_user {
                exec_args.extend(["--user".to_string(), user.to_string()]);
            }
            exec_args.extend([id, "/bin/sh".to_string()]);
            let status = std::process::Command::new("docker")
                .args(&exec_args)
                .status()
                .map_err(|e| anyhow!("Failed to run docker: {e}"))?;
            if !status.success() {
                return Err(anyhow!("`docker exec` failed"));
            }
            Ok(())
        }
        cli::Command::Unknown(msg) => Err(anyhow!(msg)),
    }
}
