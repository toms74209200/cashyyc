use crate::cli;
use crate::devcontainer;
use crate::docker;
use crate::features;
use crate::setup;
use crate::setup::ContainerTarget;
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
            println!("cyyc {}", env!("GIT_VERSION"));
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
    let config_dir = config_path.parent().unwrap_or(cwd.as_path());

    let target = setup::from_config(&config, &cwd, &config_path, config_dir);

    let (found_container, container_id): (Option<docker::Container>, Option<String>) = match &target
    {
        ContainerTarget::Compose(c) => {
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
            let running_id = docker::parse_container_id(&String::from_utf8_lossy(&output.stdout));
            let container_id = if running_id.is_some() {
                running_id
            } else {
                let all_output = std::process::Command::new("docker")
                    .args([
                        "ps", "-a", "--filter", &c.filter1, "--filter", &c.filter2, "--format",
                        "{{.ID}}",
                    ])
                    .output()
                    .map_err(|e| anyhow!("Failed to run docker: {e}"))?;
                if !all_output.status.success() {
                    let stderr = String::from_utf8_lossy(&all_output.stderr);
                    return Err(anyhow!(
                        "`docker ps` failed with status {}: {}",
                        all_output.status,
                        stderr.trim()
                    ));
                }
                let stopped_id =
                    docker::parse_container_id(&String::from_utf8_lossy(&all_output.stdout));
                if stopped_id.is_some() {
                    let mut start_args = c.global_args.clone();
                    start_args.push("start".to_string());
                    start_args.extend(c.services.iter().cloned());
                    let status = std::process::Command::new("docker")
                        .arg("compose")
                        .args(&start_args)
                        .status()
                        .map_err(|e| anyhow!("Failed to run docker: {e}"))?;
                    if !status.success() {
                        return Err(anyhow!("`docker compose start` failed"));
                    }
                }
                stopped_id
            };
            (None, container_id)
        }
        ContainerTarget::Single(_) => {
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

    let features_map = match &config {
        devcontainer::DevcontainerConfig::Image(c) => &c.common.features,
        devcontainer::DevcontainerConfig::Dockerfile(c) => &c.common.features,
        devcontainer::DevcontainerConfig::DockerfileBuild(c) => &c.common.features,
        devcontainer::DevcontainerConfig::DockerCompose(c) => &c.common.features,
    };

    let features_plan: Option<(features::InstallPlan, std::path::PathBuf)> =
        if !features_map.is_empty() {
            Some(download_features(features_map, config_dir, &cwd)?)
        } else {
            None
        };

    let target = if let Some((ref plan, ref fdir)) = features_plan {
        match target {
            ContainerTarget::Single(s) => {
                let base = match &s.dockerfile {
                    None => format!("FROM {}", s.image_tag),
                    Some(p) => std::fs::read_to_string(p)
                        .map_err(|e| anyhow!("failed to read Dockerfile: {e}"))?,
                };
                let content = features::feature_dockerfile(&base, plan);
                let dockerfile_path = fdir.join("Dockerfile.features");
                std::fs::write(&dockerfile_path, &content)
                    .map_err(|e| anyhow!("failed to write feature Dockerfile: {e}"))?;
                ContainerTarget::Single(setup::ContainerSetup {
                    image_tag: format!("{}-features", docker::image_tag(&cwd)),
                    dockerfile: Some(dockerfile_path),
                    run_args: s.run_args,
                    override_command: s.override_command,
                })
            }
            ContainerTarget::Compose(c) => {
                let base = (|| -> Option<String> {
                    let out = std::process::Command::new("docker")
                        .arg("compose")
                        .args(&c.global_args)
                        .args(["config", "--format", "json"])
                        .output()
                        .ok()?;
                    if !out.status.success() {
                        return None;
                    }
                    let cfg: devcontainer::ComposeResolved =
                        serde_json::from_slice(&out.stdout).ok()?;
                    match cfg.services.get(&c.service)?.feature_base_source() {
                        devcontainer::FeatureBaseSource::Image(img) => Some(format!("FROM {img}")),
                        devcontainer::FeatureBaseSource::DockerfilePath(p) => {
                            std::fs::read_to_string(p).ok()
                        }
                    }
                })()
                .ok_or_else(|| anyhow!("failed to resolve compose service base for features"))?;
                let content = features::feature_dockerfile(&base, plan);
                let dockerfile_path = fdir.join("Dockerfile.features");
                std::fs::write(&dockerfile_path, &content)
                    .map_err(|e| anyhow!("failed to write feature Dockerfile: {e}"))?;
                let override_content = format!(
                    "{}    build:\n      dockerfile: {}\n      context: {}\n",
                    c.override_content,
                    dockerfile_path.display(),
                    fdir.display()
                );
                ContainerTarget::Compose(devcontainer::ComposeArgs {
                    override_content,
                    ..c
                })
            }
        }
    } else {
        target
    };

    let id: String = match target {
        ContainerTarget::Single(s) => {
            if let Some((_, ref fdir)) = features_plan {
                let status = std::process::Command::new("docker")
                    .args([
                        "build",
                        "-f",
                        &s.dockerfile.as_ref().unwrap().display().to_string(),
                        "-t",
                        &s.image_tag,
                        &fdir.display().to_string(),
                    ])
                    .status()
                    .map_err(|e| anyhow!("Failed to run docker: {e}"))?;
                if !status.success() {
                    return Err(anyhow!("`docker build` for features failed"));
                }
            } else {
                match &config {
                    devcontainer::DevcontainerConfig::Image(c) => {
                        let status = std::process::Command::new("docker")
                            .args(["pull", &c.image])
                            .status()
                            .map_err(|e| anyhow!("Failed to run docker: {e}"))?;
                        if !status.success() {
                            return Err(anyhow!("`docker pull` failed"));
                        }
                    }
                    devcontainer::DevcontainerConfig::Dockerfile(c) => {
                        let build = devcontainer::normalize_dockerfile_config(c);
                        let build_args =
                            devcontainer::container_build_args(&build, config_dir, &s.image_tag);
                        let status = std::process::Command::new("docker")
                            .arg("build")
                            .args(&build_args)
                            .status()
                            .map_err(|e| anyhow!("Failed to run docker: {e}"))?;
                        if !status.success() {
                            return Err(anyhow!("`docker build` failed"));
                        }
                    }
                    devcontainer::DevcontainerConfig::DockerfileBuild(c) => {
                        let build_args =
                            devcontainer::container_build_args(&c.build, config_dir, &s.image_tag);
                        let status = std::process::Command::new("docker")
                            .arg("build")
                            .args(&build_args)
                            .status()
                            .map_err(|e| anyhow!("Failed to run docker: {e}"))?;
                        if !status.success() {
                            return Err(anyhow!("`docker build` failed"));
                        }
                    }
                    devcontainer::DevcontainerConfig::DockerCompose(_) => unreachable!(),
                }
            }
            let mut run_args = s.run_args;
            run_args.extend(["--entrypoint".to_string(), "/bin/sh".to_string()]);
            run_args.push(s.image_tag.clone());
            let image_config = if s.override_command == Some(false) {
                let output = std::process::Command::new("docker")
                    .args([
                        "image",
                        "inspect",
                        "--format",
                        "{{json .Config}}",
                        &s.image_tag,
                    ])
                    .output()
                    .map_err(|e| anyhow!("Failed to run docker: {e}"))?;
                if !output.status.success() {
                    let stderr = String::from_utf8_lossy(&output.stderr);
                    return Err(anyhow!(
                        "Failed to inspect image (overrideCommand: false): {}",
                        stderr.trim()
                    ));
                }
                docker::ImageConfig::parse(String::from_utf8_lossy(&output.stdout).trim())
            } else {
                docker::ImageConfig {
                    entrypoint: vec![],
                    cmd: vec![],
                }
            };
            run_args.extend(devcontainer::container_start_args(
                s.override_command,
                &image_config.entrypoint,
                &image_config.cmd,
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
                .ok_or_else(|| anyhow!("Failed to get container ID from `docker run`"))?
        }
        ContainerTarget::Compose(c) => {
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
                let timestamp = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_millis();
                let p = compose_dir.join(format!("{}-{}.yml", c.project_name, timestamp));
                std::fs::write(&p, &c.override_content)
                    .map_err(|e| anyhow!("Failed to write compose override file: {e}"))?;
                if !no_recreate {
                    let mut build_args = c.global_args.clone();
                    build_args.extend(["-f".to_string(), p.display().to_string()]);
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
            docker::parse_container_id(&String::from_utf8_lossy(&output.stdout))
                .ok_or_else(|| anyhow!("Failed to get container ID from `docker compose up`"))?
        }
    };
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
        devcontainer::expand_variables(&container_workspace_folder, cwd, "", &Default::default());
    let container_env: std::collections::HashMap<String, String> =
        std::process::Command::new("docker")
            .args(["exec", &id, "printenv"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| {
                String::from_utf8_lossy(&o.stdout)
                    .lines()
                    .filter_map(|line| {
                        let mut parts = line.splitn(2, '=');
                        Some((
                            parts.next()?.to_string(),
                            parts.next().unwrap_or("").to_string(),
                        ))
                    })
                    .collect()
            })
            .unwrap_or_default();
    let remote_env = match config {
        devcontainer::DevcontainerConfig::Image(c) => c.common.remote_env.as_ref(),
        devcontainer::DevcontainerConfig::Dockerfile(c) => c.common.remote_env.as_ref(),
        devcontainer::DevcontainerConfig::DockerfileBuild(c) => c.common.remote_env.as_ref(),
        devcontainer::DevcontainerConfig::DockerCompose(c) => c.common.remote_env.as_ref(),
    };
    let mut exec_args = vec!["exec".to_string(), "-it".to_string()];
    if let Some(user) = remote_user {
        exec_args.extend(["--user".to_string(), user]);
    }
    exec_args.extend(["--workdir".to_string(), container_workspace_folder.clone()]);
    if let Some(env) = remote_env {
        let mut pairs: Vec<(&String, &String)> = env
            .iter()
            .filter_map(|(k, v)| v.as_ref().map(|val| (k, val)))
            .collect();
        pairs.sort_by_key(|(k, _)| k.as_str());
        for (key, value) in pairs {
            let expanded = devcontainer::expand_variables(
                value,
                cwd,
                &container_workspace_folder,
                &container_env,
            );
            exec_args.extend(["--env".to_string(), format!("{}={}", key, expanded)]);
        }
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

fn download_features(
    features_map: &std::collections::HashMap<String, serde_json::Value>,
    devcontainer_dir: &std::path::Path,
    cwd: &std::path::Path,
) -> Result<(features::InstallPlan, std::path::PathBuf)> {
    let username = std::env::var("USER").unwrap_or_else(|_| "user".to_string());
    let features_dir = std::env::temp_dir()
        .join(format!("cyyc-{username}"))
        .join("features")
        .join(docker::image_tag(cwd));
    std::fs::create_dir_all(&features_dir)
        .map_err(|e| anyhow!("failed to create features temp dir: {e}"))?;

    let mut sorted: Vec<(&String, &serde_json::Value)> = features_map.iter().collect();
    sorted.sort_by_key(|(k, _)| k.as_str());

    let mut resolved = Vec::new();
    for (idx, (id, options)) in sorted.iter().enumerate() {
        let source = {
            let raw = features::FeatureSource::parse(id)?;
            match raw {
                features::FeatureSource::Local(p) if p.is_relative() => {
                    features::FeatureSource::Local(devcontainer_dir.join(&p))
                }
                other => other,
            }
        };
        let feature_dir = features_dir.join(idx.to_string());
        std::fs::create_dir_all(&feature_dir)
            .map_err(|e| anyhow!("failed to create feature dir: {e}"))?;
        match &source {
            features::FeatureSource::Local(path) => {
                let status = std::process::Command::new("cp")
                    .args([
                        "-r",
                        &format!("{}/.", path.display()),
                        &feature_dir.display().to_string(),
                    ])
                    .status()
                    .map_err(|e| anyhow!("failed to copy local feature: {e}"))?;
                if !status.success() {
                    return Err(anyhow!(
                        "failed to copy local feature from {}",
                        path.display()
                    ));
                }
            }
            features::FeatureSource::Tarball(url) => {
                let tarball = feature_dir.join("feature.tgz");
                let status = std::process::Command::new("curl")
                    .args(["-sfL", url, "-o", &tarball.display().to_string()])
                    .status()
                    .map_err(|e| anyhow!("failed to run curl: {e}"))?;
                if !status.success() {
                    return Err(anyhow!("failed to download feature from {url}"));
                }
                let status = std::process::Command::new("tar")
                    .args([
                        "xf",
                        &tarball.display().to_string(),
                        "-C",
                        &feature_dir.display().to_string(),
                    ])
                    .status()
                    .map_err(|e| anyhow!("failed to run tar: {e}"))?;
                if !status.success() {
                    return Err(anyhow!("failed to extract {}", tarball.display()));
                }
            }
            features::FeatureSource::Oci {
                registry,
                path,
                version,
            } => {
                let tarball = feature_dir.join("feature.tgz");
                let token = {
                    let url = format!(
                        "https://{registry}/token?scope=repository:{path}:pull&service={registry}"
                    );
                    let output = std::process::Command::new("curl")
                        .args(["-sf", &url])
                        .output()
                        .map_err(|e| anyhow!("failed to run curl: {e}"))?;
                    if !output.status.success() {
                        return Err(anyhow!("failed to fetch OCI token for {registry}/{path}"));
                    }
                    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
                        .map_err(|e| anyhow!("failed to parse OCI token response: {e}"))?;
                    json["token"]
                        .as_str()
                        .map(String::from)
                        .ok_or_else(|| anyhow!("OCI token response missing 'token' field"))?
                };
                let manifest_url = format!("https://{registry}/v2/{path}/manifests/{version}");
                let output = std::process::Command::new("curl")
                    .args([
                        "-sf",
                        "-H",
                        &format!("Authorization: Bearer {token}"),
                        "-H",
                        "Accept: application/vnd.oci.image.manifest.v1+json",
                        &manifest_url,
                    ])
                    .output()
                    .map_err(|e| anyhow!("failed to run curl: {e}"))?;
                if !output.status.success() {
                    return Err(anyhow!(
                        "failed to fetch OCI manifest for {registry}/{path}:{version}"
                    ));
                }
                let manifest: serde_json::Value = serde_json::from_slice(&output.stdout)
                    .map_err(|e| anyhow!("failed to parse OCI manifest: {e}"))?;
                let digest = manifest["layers"][0]["digest"]
                    .as_str()
                    .ok_or_else(|| anyhow!("OCI manifest missing layers[0].digest"))?;
                let blob_url = format!("https://{registry}/v2/{path}/blobs/{digest}");
                let status = std::process::Command::new("curl")
                    .args([
                        "-sfL",
                        "-H",
                        &format!("Authorization: Bearer {token}"),
                        "-o",
                        &tarball.display().to_string(),
                        &blob_url,
                    ])
                    .status()
                    .map_err(|e| anyhow!("failed to run curl: {e}"))?;
                if !status.success() {
                    return Err(anyhow!("failed to download OCI blob for {registry}/{path}"));
                }
                let status = std::process::Command::new("tar")
                    .args([
                        "xf",
                        &tarball.display().to_string(),
                        "-C",
                        &feature_dir.display().to_string(),
                    ])
                    .status()
                    .map_err(|e| anyhow!("failed to run tar: {e}"))?;
                if !status.success() {
                    return Err(anyhow!("failed to extract {}", tarball.display()));
                }
            }
        }
        let manifest_content =
            std::fs::read_to_string(feature_dir.join("devcontainer-feature.json"))
                .map_err(|e| anyhow!("devcontainer-feature.json not found in feature {id}: {e}"))?;
        let manifest = features::FeatureManifest::parse(&manifest_content)?;
        resolved.push(features::Feature {
            short_id: manifest.id,
            dir: feature_dir,
            options: (*options).clone(),
            installs_after: manifest.installs_after,
            container_env: manifest.container_env,
        });
    }

    let plan = features::InstallPlan::new(resolved)?;
    Ok((plan, features_dir))
}
