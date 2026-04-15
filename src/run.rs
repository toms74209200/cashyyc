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
            let ps_filter = format!("label=devcontainer.local_folder={}", cwd.display());
            let output = std::process::Command::new("docker")
                .args(["ps", "--filter", &ps_filter, "--format", "{{.ID}}"])
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
                let output = std::process::Command::new("docker")
                    .args(["ps", "-a", "--filter", &ps_filter, "--format", "{{.ID}}"])
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
                if let Some(id) = docker::parse_container_id(&stdout) {
                    let status = std::process::Command::new("docker")
                        .args(["start", &id])
                        .status()
                        .map_err(|e| anyhow!("Failed to run docker: {e}"))?;
                    if !status.success() {
                        return Err(anyhow!("`docker start` failed"));
                    }
                    container_id = Some(id);
                }
            }
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
            let remote_user_from_config = match &config {
                devcontainer::DevcontainerConfig::Image(c) => c.common.remote_user.clone(),
                devcontainer::DevcontainerConfig::Dockerfile(c) => c.common.remote_user.clone(),
                devcontainer::DevcontainerConfig::DockerfileBuild(c) => {
                    c.common.remote_user.clone()
                }
                devcontainer::DevcontainerConfig::DockerCompose(c) => c.common.remote_user.clone(),
            };
            let remote_user = remote_user_from_config
                .clone()
                .or_else(|| {
                    std::process::Command::new("docker")
                        .args([
                            "inspect",
                            "--format",
                            "{{index .Config.Labels \"devcontainer.metadata\"}}",
                            &id,
                        ])
                        .output()
                        .ok()
                        .and_then(|o| {
                            docker::parse_remote_user_from_metadata(
                                String::from_utf8_lossy(&o.stdout).trim(),
                            )
                        })
                })
                .or_else(|| {
                    std::process::Command::new("docker")
                        .args(["inspect", "--format", "{{.Config.User}}", &id])
                        .output()
                        .ok()
                        .and_then(|o| {
                            let user = String::from_utf8_lossy(&o.stdout).trim().to_string();
                            if user.is_empty() { None } else { Some(user) }
                        })
                });
            let shell = std::process::Command::new("docker")
                .args(["exec", &id, "printenv", "SHELL"])
                .output()
                .ok()
                .and_then(|o| {
                    let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
                    if s.is_empty() { None } else { Some(s) }
                })
                .or_else(|| {
                    remote_user.as_deref().and_then(|user| {
                        std::process::Command::new("docker")
                            .args(["exec", &id, "getent", "passwd", user])
                            .output()
                            .ok()
                            .and_then(|o| {
                                devcontainer::parse_shell_from_passwd(
                                    String::from_utf8_lossy(&o.stdout).trim(),
                                )
                            })
                    })
                })
                .unwrap_or_else(|| "/bin/sh".to_string());
            let mut exec_args = vec!["exec".to_string(), "-it".to_string()];
            if let Some(user) = remote_user {
                exec_args.extend(["--user".to_string(), user]);
            }
            exec_args.extend([id, shell]);
            let status = std::process::Command::new("docker")
                .args(&exec_args)
                .status()
                .map_err(|e| anyhow!("Failed to run docker: {e}"))?;
            if !status.success() {
                return Err(anyhow!("`docker exec` failed"));
            }
            Ok(())
        }
        cli::Command::Help => {
            println!(
                "Usage: cyyc <COMMAND>

Commands:
  shell [name]  Open a shell in the dev container
  help          Print this message
  version       Print version information

Options:
  -h, --help     Print help
  -V, --version  Print version information"
            );
            Ok(())
        }
        cli::Command::Unknown(msg) => Err(anyhow!(msg)),
    }
}
