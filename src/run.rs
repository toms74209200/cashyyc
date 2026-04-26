use crate::cli;
use crate::devcontainer;
use crate::docker;
use crate::features;
use crate::setup;
use anyhow::{Result, anyhow};

pub fn run(args: Vec<String>) -> Result<()> {
    match cli::parse_args(&args) {
        cli::Command::Shell { name } => shell(name),
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
        cli::Command::Version => {
            println!("cyyc {}", env!("CARGO_PKG_VERSION"));
            Ok(())
        }
        cli::Command::Unknown(msg) => Err(anyhow!(msg)),
    }
}

fn shell(name: Option<String>) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let devcontainer_dir = cwd.join(".devcontainer");
    let mut configs = vec![];
    let root = devcontainer_dir.join("devcontainer.json");
    if root.is_file() {
        configs.push(root);
    }
    if let Ok(entries) = std::fs::read_dir(&devcontainer_dir) {
        let mut named: Vec<_> = entries
            .flatten()
            .filter(|e| e.path().is_dir())
            .filter_map(|e| {
                let p = e.path().join("devcontainer.json");
                p.is_file().then_some(p)
            })
            .collect();
        named.sort();
        configs.extend(named);
    }
    let config_path = match (configs.as_slice(), name.as_deref()) {
        ([], _) => {
            return Err(anyhow!(
                "No devcontainer.json found in {}",
                devcontainer_dir.display()
            ));
        }
        ([c], _) => c.clone(),
        (_, Some(n)) => {
            let path = devcontainer_dir.join(n).join("devcontainer.json");
            if !path.is_file() {
                return Err(anyhow!(
                    "Dev container config ({}) not found.",
                    path.display()
                ));
            }
            path
        }
        (cs, None) => {
            let names: Vec<_> = cs
                .iter()
                .filter_map(|p| {
                    p.parent()
                        .and_then(|d| d.file_name())
                        .map(|n| n.to_string_lossy().to_string())
                })
                .collect();
            return Err(anyhow!(
                "Multiple devcontainer configs found. Specify a name: {}",
                names.join(", ")
            ));
        }
    };
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
    let compose = if let devcontainer::DevcontainerConfig::DockerCompose(ref c) = config {
        let devcontainer_dir = config_path.parent().unwrap_or(cwd.as_path());
        Some(devcontainer::compose_args(c, &cwd, devcontainer_dir))
    } else {
        None
    };

    let (found_container, container_id): (Option<docker::Container>, Option<String>) = match &config
    {
        devcontainer::DevcontainerConfig::DockerCompose(_) => {
            let c = compose.as_ref().unwrap();
            let output = std::process::Command::new("docker")
                .args([
                    "ps", "--filter", &c.filter1, "--filter", &c.filter2, "--format", "{{.ID}}",
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
            (
                None,
                docker::parse_container_id(&String::from_utf8_lossy(&output.stdout)),
            )
        }
        _ => {
            let output = std::process::Command::new("docker")
                .args([
                    "ps",
                    "--filter",
                    "label=devcontainer.config_file",
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
            let ids = docker::parse_container_ids(&String::from_utf8_lossy(&output.stdout));
            let found_running = if !ids.is_empty() {
                let inspect = std::process::Command::new("docker")
                    .arg("inspect")
                    .args(&ids)
                    .output()
                    .map_err(|e| anyhow!("Failed to run docker: {e}"))?;
                if inspect.status.success() {
                    docker::find_container(
                        String::from_utf8_lossy(&inspect.stdout).trim(),
                        &config_path,
                        &cwd,
                    )
                } else {
                    None
                }
            } else {
                None
            };
            let found = if found_running.is_some() {
                found_running
            } else {
                let output = std::process::Command::new("docker")
                    .args([
                        "ps",
                        "-a",
                        "--filter",
                        "label=devcontainer.config_file",
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
                let ids = docker::parse_container_ids(&String::from_utf8_lossy(&output.stdout));
                if !ids.is_empty() {
                    let inspect = std::process::Command::new("docker")
                        .arg("inspect")
                        .args(&ids)
                        .output()
                        .map_err(|e| anyhow!("Failed to run docker: {e}"))?;
                    if inspect.status.success() {
                        if let Some(c) = docker::find_container(
                            String::from_utf8_lossy(&inspect.stdout).trim(),
                            &config_path,
                            &cwd,
                        ) {
                            let status = std::process::Command::new("docker")
                                .args(["start", &c.id])
                                .status()
                                .map_err(|e| anyhow!("Failed to run docker: {e}"))?;
                            if !status.success() {
                                return Err(anyhow!("`docker start` failed"));
                            }
                            Some(c)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                } else {
                    None
                }
            };
            let id = found.as_ref().map(|c| c.id.clone());
            (found, id)
        }
    };

    if let Some(id) = container_id {
        return exec_in_container(id, found_container, &config, &cwd);
    }

    enum Setup {
        Single(setup::ContainerSetup),
        Compose(Option<String>),
    }

    let features_map = match &config {
        devcontainer::DevcontainerConfig::Image(c) => &c.common.features,
        devcontainer::DevcontainerConfig::Dockerfile(c) => &c.common.features,
        devcontainer::DevcontainerConfig::DockerfileBuild(c) => &c.common.features,
        devcontainer::DevcontainerConfig::DockerCompose(c) => &c.common.features,
    };
    let setup: Setup = match &config {
        devcontainer::DevcontainerConfig::Image(c) => {
            let status = std::process::Command::new("docker")
                .args(["pull", &c.image])
                .status()
                .map_err(|e| anyhow!("Failed to run docker: {e}"))?;
            if !status.success() {
                return Err(anyhow!("`docker pull` failed"));
            }
            Setup::Single(setup::from_image(c, &cwd, &config_path))
        }
        devcontainer::DevcontainerConfig::Dockerfile(c) => {
            let devcontainer_dir = config_path.parent().unwrap_or(cwd.as_path());
            let build = devcontainer::normalize_dockerfile_config(c);
            let tag = docker::image_tag(&cwd);
            let build_args = devcontainer::container_build_args(&build, devcontainer_dir, &tag);
            let status = std::process::Command::new("docker")
                .arg("build")
                .args(&build_args)
                .status()
                .map_err(|e| anyhow!("Failed to run docker: {e}"))?;
            if !status.success() {
                return Err(anyhow!("`docker build` failed"));
            }
            Setup::Single(setup::from_dockerfile(c, &cwd, &config_path))
        }
        devcontainer::DevcontainerConfig::DockerfileBuild(c) => {
            let devcontainer_dir = config_path.parent().unwrap_or(cwd.as_path());
            let tag = docker::image_tag(&cwd);
            let build_args = devcontainer::container_build_args(&c.build, devcontainer_dir, &tag);
            let status = std::process::Command::new("docker")
                .arg("build")
                .args(&build_args)
                .status()
                .map_err(|e| anyhow!("Failed to run docker: {e}"))?;
            if !status.success() {
                return Err(anyhow!("`docker build` failed"));
            }
            Setup::Single(setup::from_dockerfile_build(c, &cwd, &config_path))
        }
        devcontainer::DevcontainerConfig::DockerCompose(_) => {
            let c = compose.as_ref().unwrap();
            let username = std::env::var("USER").unwrap_or_else(|_| "user".to_string());
            let compose_dir = std::env::temp_dir()
                .join(format!("cyyc-{}", username))
                .join("docker-compose");
            std::fs::create_dir_all(&compose_dir)
                .map_err(|e| anyhow!("Failed to create compose override directory: {e}"))?;
            let existing_id = {
                let output = std::process::Command::new("docker")
                    .args([
                        "ps", "-a", "--filter", &c.filter1, "--filter", &c.filter2, "--format",
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
                docker::parse_container_id(&String::from_utf8_lossy(&output.stdout))
            };
            let persisted_override = existing_id.as_deref().and_then(|id| {
                let out = std::process::Command::new("docker")
                    .args([
                        "inspect",
                        "--format",
                        "{{index .Config.Labels \"com.docker.compose.project.config_files\"}}",
                        id,
                    ])
                    .output()
                    .ok()?;
                let config_files = String::from_utf8_lossy(&out.stdout).trim().to_string();
                config_files.split(',').find_map(|f| {
                    let p = std::path::Path::new(f.trim());
                    let is_features_override = p
                        .file_name()
                        .and_then(|n| n.to_str())
                        .map(|n| {
                            n.contains("docker-compose.devcontainer.containerFeatures")
                                || p.starts_with(&compose_dir)
                        })
                        .unwrap_or(false);
                    if is_features_override && p.exists() {
                        Some(p.to_path_buf())
                    } else {
                        None
                    }
                })
            });
            let no_recreate = existing_id.is_some();
            let override_path = if let Some(p) = persisted_override {
                p
            } else {
                if !no_recreate {
                    let mut build_args = c.global_args.clone();
                    build_args.push("build".to_string());
                    build_args.extend(c.services.iter().cloned());
                    let status = std::process::Command::new("docker")
                        .arg("compose")
                        .args(&build_args)
                        .status()
                        .map_err(|e| anyhow!("Failed to run docker: {e}"))?;
                    if !status.success() {
                        return Err(anyhow!("`docker compose build` failed"));
                    }
                }
                let timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis();
                let p = compose_dir.join(format!("{}-{}.yml", c.project_name, timestamp));
                std::fs::write(&p, &c.override_content)
                    .map_err(|e| anyhow!("Failed to write compose override file: {e}"))?;
                p
            };
            let mut up_args = c.global_args.clone();
            up_args.extend(["-f".to_string(), override_path.display().to_string()]);
            up_args.extend(["up".to_string(), "-d".to_string()]);
            if no_recreate {
                up_args.push("--no-recreate".to_string());
            }
            up_args.extend(c.services.iter().cloned());
            let status = std::process::Command::new("docker")
                .arg("compose")
                .args(&up_args)
                .status()
                .map_err(|e| anyhow!("Failed to run docker: {e}"))?;
            if !status.success() {
                return Err(anyhow!("`docker compose up` failed"));
            }
            let output = std::process::Command::new("docker")
                .args([
                    "ps", "--filter", &c.filter1, "--filter", &c.filter2, "--format", "{{.ID}}",
                ])
                .output()
                .map_err(|e| anyhow!("Failed to run docker: {e}"))?;
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(anyhow!("`docker ps` failed: {}", stderr.trim()));
            }
            Setup::Compose(docker::parse_container_id(&String::from_utf8_lossy(
                &output.stdout,
            )))
        }
    };

    let id = match setup {
        Setup::Compose(id) => id,
        Setup::Single(s) => {
            let devcontainer_dir = config_path.parent().unwrap_or(cwd.as_path());
            let final_image =
                build_with_features(&s.base_image, features_map, devcontainer_dir, &cwd)?;
            let mut run_args = s.run_args;
            run_args.extend(["--entrypoint".to_string(), "/bin/sh".to_string()]);
            run_args.push(final_image.clone());
            let (image_entrypoint, image_cmd) = if s.override_command == Some(false) {
                inspect_image_entrypoint_cmd(&final_image).map_err(|e| {
                    anyhow!("Failed to inspect image entrypoint (overrideCommand: false): {e}")
                })?
            } else {
                (vec![], vec![])
            };
            run_args.extend(devcontainer::container_start_args(
                s.override_command,
                &image_entrypoint,
                &image_cmd,
            ));
            let output = std::process::Command::new("docker")
                .arg("run")
                .args(&run_args)
                .output()
                .map_err(|e| anyhow!("Failed to run docker: {e}"))?;
            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                return Err(anyhow!("`docker run` failed: {}", stderr.trim()));
            }
            docker::parse_container_id(&String::from_utf8_lossy(&output.stdout))
        }
    };
    let id = id.ok_or_else(|| anyhow!("Failed to get container ID"))?;
    exec_in_container(id, None, &config, &cwd)
}

fn exec_in_container(
    id: String,
    found_container: Option<docker::Container>,
    config: &devcontainer::DevcontainerConfig,
    cwd: &std::path::Path,
) -> Result<()> {
    let remote_user_from_config = match config {
        devcontainer::DevcontainerConfig::Image(c) => c.common.remote_user.clone(),
        devcontainer::DevcontainerConfig::Dockerfile(c) => c.common.remote_user.clone(),
        devcontainer::DevcontainerConfig::DockerfileBuild(c) => c.common.remote_user.clone(),
        devcontainer::DevcontainerConfig::DockerCompose(c) => c.common.remote_user.clone(),
    };
    let remote_user_from_container = if let Some(ref c) = found_container {
        c.remote_user.clone()
    } else {
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
                docker::parse_remote_user_from_metadata(String::from_utf8_lossy(&o.stdout).trim())
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
            })
    };
    let remote_user = remote_user_from_config.or(remote_user_from_container);
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
    let container_workspace_folder = match config {
        devcontainer::DevcontainerConfig::Image(c) => c.common.workspace_folder.clone(),
        devcontainer::DevcontainerConfig::Dockerfile(c) => c.common.workspace_folder.clone(),
        devcontainer::DevcontainerConfig::DockerfileBuild(c) => c.common.workspace_folder.clone(),
        devcontainer::DevcontainerConfig::DockerCompose(c) => Some(c.workspace_folder.clone()),
    }
    .unwrap_or_else(|| {
        format!(
            "/workspaces/{}",
            cwd.file_name().unwrap_or_default().to_string_lossy()
        )
    });
    let container_workspace_folder =
        devcontainer::expand_variables(&container_workspace_folder, cwd, "");
    let mut exec_args = vec!["exec".to_string(), "-it".to_string()];
    if let Some(user) = remote_user {
        exec_args.extend(["--user".to_string(), user]);
    }
    exec_args.extend(["--workdir".to_string(), container_workspace_folder]);
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

fn build_with_features(
    base_image: &str,
    features_map: &std::collections::HashMap<String, serde_json::Value>,
    devcontainer_dir: &std::path::Path,
    cwd: &std::path::Path,
) -> Result<String> {
    let username = std::env::var("USER").unwrap_or_else(|_| "user".to_string());
    let features_dir = std::env::temp_dir()
        .join(format!("cyyc-{username}"))
        .join("features")
        .join(docker::image_tag(cwd));
    std::fs::create_dir_all(&features_dir)
        .map_err(|e| anyhow!("failed to create features temp dir: {e}"))?;

    let Some(dockerfile_path) =
        features::prepare(base_image, features_map, devcontainer_dir, &features_dir)?
    else {
        return Ok(base_image.to_string());
    };

    let tag = format!("{}-features", docker::image_tag(cwd));
    let status = std::process::Command::new("docker")
        .args([
            "build",
            "-f",
            &dockerfile_path.display().to_string(),
            "-t",
            &tag,
            &features_dir.display().to_string(),
        ])
        .status()
        .map_err(|e| anyhow!("failed to run docker: {e}"))?;
    if !status.success() {
        return Err(anyhow!("`docker build` for features failed"));
    }

    Ok(tag)
}

fn inspect_image_entrypoint_cmd(image: &str) -> Result<(Vec<String>, Vec<String>)> {
    let entrypoint = {
        let output = std::process::Command::new("docker")
            .args([
                "image",
                "inspect",
                "--format",
                "{{json .Config.Entrypoint}}",
                image,
            ])
            .output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("`docker image inspect` failed: {}", stderr.trim()));
        }
        docker::parse_image_config_json(&String::from_utf8_lossy(&output.stdout))
    };
    let cmd = {
        let output = std::process::Command::new("docker")
            .args([
                "image",
                "inspect",
                "--format",
                "{{json .Config.Cmd}}",
                image,
            ])
            .output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow!("`docker image inspect` failed: {}", stderr.trim()));
        }
        docker::parse_image_config_json(&String::from_utf8_lossy(&output.stdout))
    };
    Ok((entrypoint, cmd))
}
