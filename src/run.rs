use crate::cli;
use crate::devcontainer;
use crate::docker;
use crate::docker::{Docker, DockerCli};
use crate::err;
use crate::error::Result;
use crate::features;
use crate::host_exec::{Host, HostCmd, ProcessHost};
use crate::lifecycle::LifecycleCmd;
use crate::oci;
use crate::registry::{CurlRegistry, Registry};
use crate::setup;
use crate::setup::ContainerTarget;
use crate::tui::{Terminal, TuiTerminal};
use crate::uid::{UidContext, UidUpdate};

fn expand_lifecycle_cmd(
    cmd: &LifecycleCmd,
    cwd: &std::path::Path,
    container_workspace_folder: &str,
    container_env: &std::collections::HashMap<String, String>,
    local_env: &std::collections::HashMap<String, String>,
) -> LifecycleCmd {
    match cmd {
        LifecycleCmd::Shell(s) => LifecycleCmd::Shell(devcontainer::expand_variables(
            s,
            cwd,
            container_workspace_folder,
            container_env,
            local_env,
        )),
        LifecycleCmd::Exec(args) => LifecycleCmd::Exec(
            args.iter()
                .map(|a| {
                    devcontainer::expand_variables(
                        a,
                        cwd,
                        container_workspace_folder,
                        container_env,
                        local_env,
                    )
                })
                .collect(),
        ),
        LifecycleCmd::Parallel(cmds) => LifecycleCmd::Parallel(
            cmds.iter()
                .map(|c| {
                    expand_lifecycle_cmd(
                        c,
                        cwd,
                        container_workspace_folder,
                        container_env,
                        local_env,
                    )
                })
                .collect(),
        ),
    }
}

fn local_env_snapshot() -> std::collections::HashMap<String, String> {
    std::env::vars().collect()
}

pub fn run(args: Vec<String>) -> Result<()> {
    let mut docker = DockerCli;
    let mut reg = CurlRegistry;
    let mut host = ProcessHost;
    let mut term = TuiTerminal;
    match cli::parse_args(&args) {
        cli::Command::Shell { name } => shell(&mut docker, &mut reg, &mut host, name),
        cli::Command::Stop { name } => stop(&mut docker, name),
        cli::Command::Down { name } => down(&mut docker, name),
        cli::Command::Ps { name } => ps(&mut docker, name),
        cli::Command::New => new(&mut reg, &mut term),
        cli::Command::Help => {
            println!(
                "Usage: cyyc <COMMAND>

Commands:
  shell [name]  Open a shell in the dev container
  stop [name]   Stop the dev container (keeps it for reuse)
  down [name]   Remove the dev container
  ps [name]     List dev container configs and their statuses
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
        cli::Command::Unknown(msg) => Err(crate::error::Error::new(msg)),
    }
}

fn new(reg: &mut impl Registry, term: &mut impl Terminal) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let target_path = cwd.join(".devcontainer").join("devcontainer.json");
    if target_path.exists() {
        return Err(err!("{} already exists", target_path.display()));
    }

    let username = std::env::var("USER").unwrap_or_else(|_| "user".to_string());
    let tmp_dir = std::env::temp_dir()
        .join(format!("cyyc-{username}"))
        .join("new");
    std::fs::create_dir_all(&tmp_dir).map_err(|e| err!("failed to create temp dir: {e}"))?;

    let token = {
        let output = reg.fetch(
            "https://ghcr.io/token?scope=repository:devcontainers/templates:pull&service=ghcr.io",
        )?;
        if !output.success {
            return Err(err!("failed to fetch OCI token for templates"));
        }
        let json = devcontainer::jsonc::parse(&String::from_utf8_lossy(&output.body))
            .map_err(|e| err!("failed to parse OCI token response: {e}"))?;
        json.get("token")
            .and_then(|v| v.as_str())
            .map(String::from)
            .ok_or_else(|| err!("OCI token response missing 'token' field"))?
    };

    let manifest_output = reg.fetch_manifest(
        "https://ghcr.io/v2/devcontainers/templates/manifests/latest",
        &token,
    )?;
    if !manifest_output.success {
        return Err(err!("failed to fetch template collection manifest"));
    }
    let manifest = devcontainer::jsonc::parse(&String::from_utf8_lossy(&manifest_output.body))
        .map_err(|e| err!("failed to parse template collection manifest: {e}"))?;
    let digest = manifest
        .get("layers")
        .and_then(|l| l.as_array())
        .and_then(|l| l.first())
        .and_then(|l| l.get("digest"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| err!("template collection manifest missing layers[0].digest"))?;

    let collection_output = reg.fetch_blob(
        &format!("https://ghcr.io/v2/devcontainers/templates/blobs/{digest}"),
        &token,
    )?;
    if !collection_output.success {
        return Err(err!("failed to download template collection blob"));
    }

    let collection_json = String::from_utf8(collection_output.body)
        .map_err(|e| err!("template collection is not valid UTF-8: {e}"))?;
    let templates = oci::parse_templates(&collection_json);
    if templates.is_empty() {
        return Err(err!("no templates found in collection"));
    }

    let template_names: Vec<String> = templates.iter().map(|t| t.id.clone()).collect();
    let selected_idx = term
        .select("Template", &template_names)?
        .ok_or_else(|| err!("cancelled"))?;
    let selected_template = &templates[selected_idx];

    let template_token = {
        let scope = format!(
            "repository:devcontainers/templates/{}:pull",
            selected_template.id
        );
        let output = reg.fetch(&format!(
            "https://ghcr.io/token?scope={scope}&service=ghcr.io"
        ))?;
        if !output.success {
            return Err(err!(
                "failed to fetch OCI token for template {}",
                selected_template.id
            ));
        }
        let json = devcontainer::jsonc::parse(&String::from_utf8_lossy(&output.body))
            .map_err(|e| err!("failed to parse OCI token response: {e}"))?;
        json.get("token")
            .and_then(|v| v.as_str())
            .map(String::from)
            .ok_or_else(|| err!("OCI token response missing 'token' field"))?
    };

    let manifest_output = reg.fetch_manifest(
        &format!(
            "https://ghcr.io/v2/devcontainers/templates/{}/manifests/latest",
            selected_template.id
        ),
        &template_token,
    )?;
    if !manifest_output.success {
        return Err(err!(
            "failed to fetch manifest for template {}",
            selected_template.id
        ));
    }
    let manifest = devcontainer::jsonc::parse(&String::from_utf8_lossy(&manifest_output.body))
        .map_err(|e| err!("failed to parse template manifest: {e}"))?;
    let digest = manifest
        .get("layers")
        .and_then(|l| l.as_array())
        .and_then(|l| l.first())
        .and_then(|l| l.get("digest"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| err!("template manifest missing layers[0].digest"))?;

    let template_tar = tmp_dir.join("template.tar");
    let downloaded = reg.download_blob(
        &format!(
            "https://ghcr.io/v2/devcontainers/templates/{}/blobs/{digest}",
            selected_template.id
        ),
        &template_token,
        &template_tar.display().to_string(),
    )?;
    if !downloaded {
        return Err(err!("failed to download template {}", selected_template.id));
    }

    let template_dir = tmp_dir.join("template");
    std::fs::create_dir_all(&template_dir).map_err(|e| err!("failed to create temp dir: {e}"))?;
    if !reg.unpack_tar(
        &template_tar.display().to_string(),
        &template_dir.display().to_string(),
    )? {
        return Err(err!("failed to extract template"));
    }

    let template_json =
        std::fs::read_to_string(template_dir.join(".devcontainer/devcontainer.json"))
            .map_err(|e| err!("devcontainer.json not found in template: {e}"))?;

    let feature_token = {
        let output = reg.fetch(
            "https://ghcr.io/token?scope=repository:devcontainers/features:pull&service=ghcr.io",
        )?;
        if !output.success {
            return Err(err!("failed to fetch OCI token for features"));
        }
        let json = devcontainer::jsonc::parse(&String::from_utf8_lossy(&output.body))
            .map_err(|e| err!("failed to parse OCI token response: {e}"))?;
        json.get("token")
            .and_then(|v| v.as_str())
            .map(String::from)
            .ok_or_else(|| err!("OCI token response missing 'token' field"))?
    };

    let manifest_output = reg.fetch_manifest(
        "https://ghcr.io/v2/devcontainers/features/manifests/latest",
        &feature_token,
    )?;
    if !manifest_output.success {
        return Err(err!("failed to fetch feature collection manifest"));
    }
    let manifest = devcontainer::jsonc::parse(&String::from_utf8_lossy(&manifest_output.body))
        .map_err(|e| err!("failed to parse feature collection manifest: {e}"))?;
    let digest = manifest
        .get("layers")
        .and_then(|l| l.as_array())
        .and_then(|l| l.first())
        .and_then(|l| l.get("digest"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| err!("feature collection manifest missing layers[0].digest"))?;

    let feature_collection_output = reg.fetch_blob(
        &format!("https://ghcr.io/v2/devcontainers/features/blobs/{digest}"),
        &feature_token,
    )?;
    if !feature_collection_output.success {
        return Err(err!("failed to download feature collection blob"));
    }

    let feature_json = String::from_utf8(feature_collection_output.body)
        .map_err(|e| err!("feature collection is not valid UTF-8: {e}"))?;
    let features = oci::parse_features(&feature_json);

    let feature_ids: Vec<String> = if features.is_empty() {
        vec![]
    } else {
        let feature_names: Vec<String> = features.iter().map(|f| f.id.clone()).collect();
        match term.multi_select("Features", &feature_names)? {
            Some(indices) => indices.iter().map(|&i| features[i].id.clone()).collect(),
            None => return Err(err!("cancelled")),
        }
    };

    let output = oci::build_devcontainer_json(&template_json, &feature_ids)?;
    std::fs::create_dir_all(target_path.parent().unwrap())
        .map_err(|e| err!("failed to create .devcontainer directory: {e}"))?;
    std::fs::write(&target_path, &output)
        .map_err(|e| err!("failed to write {}: {e}", target_path.display()))?;

    Ok(())
}

fn shell(
    docker: &mut impl Docker,
    reg: &mut impl Registry,
    host: &mut impl Host,
    name: Option<String>,
) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let local_env = local_env_snapshot();
    let (config_path, config) = open_config(&cwd, name.as_deref())?;
    let config_dir = config_path.parent().unwrap_or(cwd.as_path());

    let target = setup::from_config(&config, &cwd, &config_path, config_dir, &local_env);

    let (found_container, container_id) =
        match lookup_existing(docker, &target, &config_path, &cwd)? {
            Existing::Running { id, meta } => (meta, Some(id)),
            Existing::Stopped { id, meta } => {
                start_existing(docker, &target, &id)?;
                let started_at = docker
                    .inspect_format(&id, "{{.State.StartedAt}}")
                    .map(|o| o.stdout.trim().to_string())?;
                let wait_for = config.common().wait_for.clone();
                if let Some(value) = config.common().post_start_command.as_ref()
                    && let Ok(cmd) = LifecycleCmd::try_from(value)
                {
                    let workdir = config.workspace_folder(&cwd, &local_env);
                    let cmd =
                        expand_lifecycle_cmd(&cmd, &cwd, &workdir, &Default::default(), &local_env);
                    let expanded_remote_user = resolve_lifecycle_user(docker, &config, &id, &cwd);
                    run_lifecycle_in_container(
                        docker,
                        &cmd,
                        &id,
                        &workdir,
                        "postStartCommand",
                        LifecycleMarker::Once(&started_at),
                        expanded_remote_user.as_deref(),
                        wait_for
                            .as_ref()
                            .is_none_or(|wf| wf.requires(&devcontainer::WaitFor::PostStartCommand)),
                    )?;
                }
                (meta, Some(id))
            }
            Existing::None => (None, None),
        };

    if let Some(id) = container_id {
        let wait_for = config.common().wait_for.clone();
        if let Some(value) = config.common().post_attach_command.as_ref()
            && let Ok(cmd) = LifecycleCmd::try_from(value)
        {
            let workdir = config.workspace_folder(&cwd, &local_env);
            let cmd = expand_lifecycle_cmd(&cmd, &cwd, &workdir, &Default::default(), &local_env);
            let expanded_remote_user = resolve_lifecycle_user(docker, &config, &id, &cwd);
            run_lifecycle_in_container(
                docker,
                &cmd,
                &id,
                &workdir,
                "postAttachCommand",
                LifecycleMarker::Always,
                expanded_remote_user.as_deref(),
                wait_for
                    .as_ref()
                    .is_none_or(|wf| wf.requires(&devcontainer::WaitFor::PostAttachCommand)),
            )?;
        }
        return exec_in_container(docker, id, found_container, &config, &cwd);
    }

    let features_map = &config.common().features;

    let features_plan: Option<(features::InstallPlan, std::path::PathBuf)> =
        if !features_map.is_empty() {
            Some(download_features(
                reg,
                host,
                features_map,
                &config.common().override_feature_install_order,
                config_dir,
                &cwd,
            )?)
        } else {
            None
        };

    let feature_users = features::FeatureInstallUsers::new(
        config.common().container_user.as_deref(),
        config.common().remote_user.as_deref(),
    );
    let target = if let Some((ref plan, ref fdir)) = features_plan {
        match target {
            ContainerTarget::Single(s) => {
                let base = match &s.dockerfile {
                    None => format!("FROM {}", s.image_tag),
                    Some(p) => std::fs::read_to_string(p)
                        .map_err(|e| err!("failed to read Dockerfile: {e}"))?,
                };
                let content = features::feature_dockerfile(&base, plan, &feature_users);
                let dockerfile_path = fdir.join("Dockerfile.features");
                std::fs::write(&dockerfile_path, &content)
                    .map_err(|e| err!("failed to write feature Dockerfile: {e}"))?;
                let mut run_args = s.run_args;
                if plan.features().iter().any(|f| f.privileged == Some(true)) {
                    run_args.push("--privileged".to_string());
                }
                if plan.features().iter().any(|f| f.init == Some(true)) {
                    run_args.push("--init".to_string());
                }
                for cap in plan.features().iter().flat_map(|f| &f.cap_add) {
                    run_args.push("--cap-add".to_string());
                    run_args.push(cap.clone());
                }
                for opt in plan.features().iter().flat_map(|f| &f.security_opt) {
                    run_args.push("--security-opt".to_string());
                    run_args.push(opt.clone());
                }
                for mount in plan.features().iter().flat_map(|f| &f.mounts) {
                    run_args.push("--mount".to_string());
                    run_args.push(mount.to_docker_arg());
                }
                ContainerTarget::Single(setup::ContainerSetup {
                    image_tag: format!("{}-features", docker::image_tag(&cwd)),
                    dockerfile: Some(dockerfile_path),
                    run_args,
                    override_command: s.override_command,
                })
            }
            ContainerTarget::Compose(c) => {
                let base = (|| -> Option<String> {
                    let out = docker.compose_config_json(&c.global_args).ok()?;
                    if !out.success {
                        return None;
                    }
                    let cfg = devcontainer::ComposeResolved::parse(&out.stdout)?;
                    match cfg.services.get(&c.service)?.feature_base_source() {
                        devcontainer::FeatureBaseSource::Image(img) => Some(format!("FROM {img}")),
                        devcontainer::FeatureBaseSource::DockerfilePath(p) => {
                            std::fs::read_to_string(p).ok()
                        }
                    }
                })()
                .ok_or_else(|| err!("failed to resolve compose service base for features"))?;
                let content = features::feature_dockerfile(&base, plan, &feature_users);
                let dockerfile_path = fdir.join("Dockerfile.features");
                std::fs::write(&dockerfile_path, &content)
                    .map_err(|e| err!("failed to write feature Dockerfile: {e}"))?;
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

    if let Some(value) = config.common().initialize_command.as_ref()
        && let Ok(cmd) = LifecycleCmd::try_from(value)
    {
        let workdir = config.workspace_folder(&cwd, &local_env);
        let cmd = expand_lifecycle_cmd(&cmd, &cwd, &workdir, &Default::default(), &local_env);
        let cmds: Vec<HostCmd> = match &cmd {
            LifecycleCmd::Shell(s) => vec![HostCmd::Shell(s.clone())],
            LifecycleCmd::Exec(args) => vec![HostCmd::Exec(args.clone())],
            LifecycleCmd::Parallel(cmds) => cmds
                .iter()
                .map(|c| match c {
                    LifecycleCmd::Shell(s) => HostCmd::Shell(s.clone()),
                    LifecycleCmd::Exec(args) => HostCmd::Exec(args.clone()),
                    LifecycleCmd::Parallel(_) => unreachable!(),
                })
                .collect(),
        };
        if !host.run_group(&cmds, "initializeCommand")? {
            return Err(err!("initializeCommand failed"));
        }
    }

    let id: String = match target {
        ContainerTarget::Single(s) => {
            if let Some((_, ref fdir)) = features_plan {
                let build_args = [
                    "-f",
                    &s.dockerfile.as_ref().unwrap().display().to_string(),
                    "-t",
                    &s.image_tag,
                    &fdir.display().to_string(),
                ]
                .map(String::from);
                if !docker.build_streamed(&build_args)? {
                    return Err(err!("`docker build` for features failed"));
                }
            } else {
                match &config {
                    devcontainer::DevcontainerConfig::Image(c) => {
                        if !docker.pull_streamed(&c.image)? {
                            return Err(err!("`docker pull` failed"));
                        }
                    }
                    devcontainer::DevcontainerConfig::Dockerfile(c) => {
                        let build = devcontainer::normalize_dockerfile_config(c);
                        let build_args =
                            devcontainer::container_build_args(&build, config_dir, &s.image_tag);
                        if !docker.build_streamed(&build_args)? {
                            return Err(err!("`docker build` failed"));
                        }
                    }
                    devcontainer::DevcontainerConfig::DockerfileBuild(c) => {
                        let build_args =
                            devcontainer::container_build_args(&c.build, config_dir, &s.image_tag);
                        if !docker.build_streamed(&build_args)? {
                            return Err(err!("`docker build` failed"));
                        }
                    }
                    devcontainer::DevcontainerConfig::DockerCompose(_) => unreachable!(),
                }
            }
            #[cfg(target_os = "linux")]
            let image_tag = {
                use std::os::unix::fs::MetadataExt;
                let image_user = docker
                    .inspect_format(&s.image_tag, "{{.Config.User}}")
                    .ok()
                    .filter(|o| o.success)
                    .map(|o| o.stdout.trim().to_string())
                    .unwrap_or_default();
                let metadata_remote_user = docker
                    .inspect_format(
                        &s.image_tag,
                        "{{index .Config.Labels \"devcontainer.metadata\"}}",
                    )
                    .ok()
                    .and_then(|o| docker::parse_remote_user_from_metadata(o.stdout.trim()));
                let meta = std::fs::metadata("/proc/self")
                    .map_err(|e| err!("Failed to get process metadata: {e}"))?;
                let host_uid = meta.uid();
                let host_gid = meta.gid();
                match UidUpdate::resolve(
                    UidContext::Single {
                        base_image: &s.image_tag,
                        image_user: &image_user,
                        metadata_remote_user: metadata_remote_user.as_deref(),
                    },
                    config.common(),
                    &s.run_args,
                    host_uid,
                    host_gid,
                    &cwd,
                ) {
                    Some(update) => {
                        if let UidUpdate::Single {
                            uid_tag,
                            remote_user,
                            new_uid,
                            new_gid,
                            image_user: img_user,
                        } = &update
                        {
                            run_uid_docker_build(
                                docker,
                                uid_tag,
                                remote_user,
                                *new_uid,
                                *new_gid,
                                img_user,
                                &s.image_tag,
                            )?;
                        }
                        update.uid_tag().to_string()
                    }
                    None => s.image_tag.clone(),
                }
            };
            #[cfg(not(target_os = "linux"))]
            let image_tag = s.image_tag.clone();

            let mut run_args = s.run_args;
            run_args.extend(["--entrypoint".to_string(), "/bin/sh".to_string()]);
            run_args.push(image_tag.clone());
            let image_config = if s.override_command == Some(false) {
                let output = docker.image_config(&image_tag)?;
                if !output.success {
                    return Err(err!(
                        "Failed to inspect image (overrideCommand: false): {}",
                        output.stderr.trim()
                    ));
                }
                docker::ImageConfig::parse(output.stdout.trim())
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
            let output = docker.run_container(&run_args)?;
            if !output.success {
                return Err(err!("`docker run` failed: {}", output.stderr.trim()));
            }
            docker::parse_container_id(&output.stdout)
                .ok_or_else(|| err!("Failed to get container ID from `docker run`"))?
        }
        ContainerTarget::Compose(c) => {
            let username = std::env::var("USER").unwrap_or_else(|_| "user".to_string());
            let compose_dir = std::env::temp_dir()
                .join(format!("cyyc-{}", username))
                .join("docker-compose");
            std::fs::create_dir_all(&compose_dir)
                .map_err(|e| err!("Failed to create compose override directory: {e}"))?;
            let existing_id = {
                let output = docker.ps_ids(&[c.filter1.clone(), c.filter2.clone()], true)?;
                if !output.success {
                    return Err(err!(
                        "`docker ps` failed with status {}: {}",
                        output.status,
                        output.stderr.trim()
                    ));
                }
                docker::parse_container_id(&output.stdout)
            };
            let persisted_override = existing_id.as_deref().and_then(|id| {
                let out = docker
                    .inspect_format(
                        id,
                        "{{index .Config.Labels \"com.docker.compose.project.config_files\"}}",
                    )
                    .ok()?;
                let config_files = out.stdout.trim().to_string();
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
                #[cfg(target_os = "linux")]
                let override_content = if !no_recreate {
                    (|| -> Option<String> {
                        use std::os::unix::fs::MetadataExt;
                        let out = docker
                            .compose_config_json(&c.global_args)
                            .ok()
                            .filter(|o| o.success)?;
                        let cfg = devcontainer::ComposeResolved::parse(&out.stdout)?;
                        let image = match cfg.services.get(&c.service)?.feature_base_source() {
                            devcontainer::FeatureBaseSource::Image(img) => img,
                            devcontainer::FeatureBaseSource::DockerfilePath(_) => return None,
                        };
                        let image_user = docker
                            .inspect_format(&image, "{{.Config.User}}")
                            .ok()
                            .filter(|o| o.success)
                            .map(|o| o.stdout.trim().to_string())
                            .unwrap_or_default();
                        let metadata_remote_user = docker
                            .inspect_format(
                                &image,
                                "{{index .Config.Labels \"devcontainer.metadata\"}}",
                            )
                            .ok()
                            .and_then(|o| docker::parse_remote_user_from_metadata(o.stdout.trim()));
                        let meta = std::fs::metadata("/proc/self").ok()?;
                        let host_uid = meta.uid();
                        let host_gid = meta.gid();
                        let update = UidUpdate::resolve(
                            UidContext::Compose {
                                override_content: &c.override_content,
                                service: &c.service,
                                image: &image,
                                image_user: &image_user,
                                metadata_remote_user: metadata_remote_user.as_deref(),
                            },
                            config.common(),
                            &[],
                            host_uid,
                            host_gid,
                            &cwd,
                        )?;
                        if let UidUpdate::Compose {
                            uid_tag,
                            remote_user,
                            new_uid,
                            new_gid,
                            image_user: img_user,
                            override_content: new_content,
                        } = update
                        {
                            run_uid_docker_build(
                                docker,
                                &uid_tag,
                                &remote_user,
                                new_uid,
                                new_gid,
                                &img_user,
                                &image,
                            )
                            .ok()?;
                            Some(new_content)
                        } else {
                            None
                        }
                    })()
                    .unwrap_or_else(|| c.override_content.clone())
                } else {
                    c.override_content.clone()
                };
                #[cfg(not(target_os = "linux"))]
                let override_content = c.override_content.clone();
                std::fs::write(&p, &override_content)
                    .map_err(|e| err!("Failed to write compose override file: {e}"))?;
                if !no_recreate {
                    let mut build_args = c.global_args.clone();
                    build_args.extend(["-f".to_string(), p.display().to_string()]);
                    build_args.push("build".to_string());
                    build_args.extend(c.services.iter().cloned());
                    if !docker.compose_build_streamed(&build_args)? {
                        return Err(err!("`docker compose build` failed"));
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
            if !docker.compose(&up_args)? {
                return Err(err!("`docker compose up` failed"));
            }
            let output = docker.ps_ids(&[c.filter1.clone(), c.filter2.clone()], false)?;
            if !output.success {
                return Err(err!("`docker ps` failed: {}", output.stderr.trim()));
            }
            docker::parse_container_id(&output.stdout)
                .ok_or_else(|| err!("Failed to get container ID from `docker compose up`"))?
        }
    };
    let created_at = docker
        .inspect_format(&id, "{{.Created}}")
        .map(|o| o.stdout.trim().to_string())?;
    let started_at = docker
        .inspect_format(&id, "{{.State.StartedAt}}")
        .map(|o| o.stdout.trim().to_string())?;
    let wait_for = config.common().wait_for.clone();
    if let Some(value) = config.common().on_create_command.as_ref()
        && let Ok(cmd) = LifecycleCmd::try_from(value)
    {
        let workdir = config.workspace_folder(&cwd, &local_env);
        let cmd = expand_lifecycle_cmd(&cmd, &cwd, &workdir, &Default::default(), &local_env);
        let expanded_remote_user = resolve_lifecycle_user(docker, &config, &id, &cwd);
        run_lifecycle_in_container(
            docker,
            &cmd,
            &id,
            &workdir,
            "onCreateCommand",
            LifecycleMarker::Once(&created_at),
            expanded_remote_user.as_deref(),
            wait_for
                .as_ref()
                .is_none_or(|wf| wf.requires(&devcontainer::WaitFor::OnCreateCommand)),
        )?;
    }
    if let Some((ref plan, _)) = features_plan {
        for feature in plan.features() {
            if let Some(value) = feature.on_create_command.as_ref()
                && let Ok(cmd) = LifecycleCmd::try_from(value)
            {
                let workdir = config.workspace_folder(&cwd, &local_env);
                let cmd =
                    expand_lifecycle_cmd(&cmd, &cwd, &workdir, &Default::default(), &local_env);
                let expanded_remote_user = resolve_lifecycle_user(docker, &config, &id, &cwd);
                run_lifecycle_in_container(
                    docker,
                    &cmd,
                    &id,
                    &workdir,
                    "onCreateCommand",
                    LifecycleMarker::Once(&created_at),
                    expanded_remote_user.as_deref(),
                    wait_for
                        .as_ref()
                        .is_none_or(|wf| wf.requires(&devcontainer::WaitFor::OnCreateCommand)),
                )?;
            }
        }
    }
    if let Some(value) = config.common().update_content_command.as_ref()
        && let Ok(cmd) = LifecycleCmd::try_from(value)
    {
        let workdir = config.workspace_folder(&cwd, &local_env);
        let cmd = expand_lifecycle_cmd(&cmd, &cwd, &workdir, &Default::default(), &local_env);
        let expanded_remote_user = resolve_lifecycle_user(docker, &config, &id, &cwd);
        run_lifecycle_in_container(
            docker,
            &cmd,
            &id,
            &workdir,
            "updateContentCommand",
            LifecycleMarker::Once(&created_at),
            expanded_remote_user.as_deref(),
            wait_for
                .as_ref()
                .is_none_or(|wf| wf.requires(&devcontainer::WaitFor::UpdateContentCommand)),
        )?;
    }
    if let Some((ref plan, _)) = features_plan {
        for feature in plan.features() {
            if let Some(value) = feature.update_content_command.as_ref()
                && let Ok(cmd) = LifecycleCmd::try_from(value)
            {
                let workdir = config.workspace_folder(&cwd, &local_env);
                let cmd =
                    expand_lifecycle_cmd(&cmd, &cwd, &workdir, &Default::default(), &local_env);
                let expanded_remote_user = resolve_lifecycle_user(docker, &config, &id, &cwd);
                run_lifecycle_in_container(
                    docker,
                    &cmd,
                    &id,
                    &workdir,
                    "updateContentCommand",
                    LifecycleMarker::Once(&created_at),
                    expanded_remote_user.as_deref(),
                    wait_for
                        .as_ref()
                        .is_none_or(|wf| wf.requires(&devcontainer::WaitFor::UpdateContentCommand)),
                )?;
            }
        }
    }
    if let Some(value) = config.common().post_create_command.as_ref()
        && let Ok(cmd) = LifecycleCmd::try_from(value)
    {
        let workdir = config.workspace_folder(&cwd, &local_env);
        let cmd = expand_lifecycle_cmd(&cmd, &cwd, &workdir, &Default::default(), &local_env);
        let expanded_remote_user = resolve_lifecycle_user(docker, &config, &id, &cwd);
        run_lifecycle_in_container(
            docker,
            &cmd,
            &id,
            &workdir,
            "postCreateCommand",
            LifecycleMarker::Once(&created_at),
            expanded_remote_user.as_deref(),
            wait_for
                .as_ref()
                .is_none_or(|wf| wf.requires(&devcontainer::WaitFor::PostCreateCommand)),
        )?;
    }
    if let Some((ref plan, _)) = features_plan {
        for feature in plan.features() {
            if let Some(value) = feature.post_create_command.as_ref()
                && let Ok(cmd) = LifecycleCmd::try_from(value)
            {
                let workdir = config.workspace_folder(&cwd, &local_env);
                let cmd =
                    expand_lifecycle_cmd(&cmd, &cwd, &workdir, &Default::default(), &local_env);
                let expanded_remote_user = resolve_lifecycle_user(docker, &config, &id, &cwd);
                run_lifecycle_in_container(
                    docker,
                    &cmd,
                    &id,
                    &workdir,
                    "postCreateCommand",
                    LifecycleMarker::Once(&created_at),
                    expanded_remote_user.as_deref(),
                    wait_for
                        .as_ref()
                        .is_none_or(|wf| wf.requires(&devcontainer::WaitFor::PostCreateCommand)),
                )?;
            }
        }
    }
    if let Some(value) = config.common().post_start_command.as_ref()
        && let Ok(cmd) = LifecycleCmd::try_from(value)
    {
        let workdir = config.workspace_folder(&cwd, &local_env);
        let cmd = expand_lifecycle_cmd(&cmd, &cwd, &workdir, &Default::default(), &local_env);
        let expanded_remote_user = resolve_lifecycle_user(docker, &config, &id, &cwd);
        run_lifecycle_in_container(
            docker,
            &cmd,
            &id,
            &workdir,
            "postStartCommand",
            LifecycleMarker::Once(&started_at),
            expanded_remote_user.as_deref(),
            wait_for
                .as_ref()
                .is_none_or(|wf| wf.requires(&devcontainer::WaitFor::PostStartCommand)),
        )?;
    }
    if let Some((ref plan, _)) = features_plan {
        for feature in plan.features() {
            if let Some(value) = feature.post_start_command.as_ref()
                && let Ok(cmd) = LifecycleCmd::try_from(value)
            {
                let workdir = config.workspace_folder(&cwd, &local_env);
                let cmd =
                    expand_lifecycle_cmd(&cmd, &cwd, &workdir, &Default::default(), &local_env);
                let expanded_remote_user = resolve_lifecycle_user(docker, &config, &id, &cwd);
                run_lifecycle_in_container(
                    docker,
                    &cmd,
                    &id,
                    &workdir,
                    "postStartCommand",
                    LifecycleMarker::Once(&started_at),
                    expanded_remote_user.as_deref(),
                    wait_for
                        .as_ref()
                        .is_none_or(|wf| wf.requires(&devcontainer::WaitFor::PostStartCommand)),
                )?;
            }
        }
    }
    if let Some(value) = config.common().post_attach_command.as_ref()
        && let Ok(cmd) = LifecycleCmd::try_from(value)
    {
        let workdir = config.workspace_folder(&cwd, &local_env);
        let cmd = expand_lifecycle_cmd(&cmd, &cwd, &workdir, &Default::default(), &local_env);
        let expanded_remote_user = resolve_lifecycle_user(docker, &config, &id, &cwd);
        run_lifecycle_in_container(
            docker,
            &cmd,
            &id,
            &workdir,
            "postAttachCommand",
            LifecycleMarker::Always,
            expanded_remote_user.as_deref(),
            wait_for
                .as_ref()
                .is_none_or(|wf| wf.requires(&devcontainer::WaitFor::PostAttachCommand)),
        )?;
    }
    if let Some((ref plan, _)) = features_plan {
        for feature in plan.features() {
            if let Some(value) = feature.post_attach_command.as_ref()
                && let Ok(cmd) = LifecycleCmd::try_from(value)
            {
                let workdir = config.workspace_folder(&cwd, &local_env);
                let cmd =
                    expand_lifecycle_cmd(&cmd, &cwd, &workdir, &Default::default(), &local_env);
                let expanded_remote_user = resolve_lifecycle_user(docker, &config, &id, &cwd);
                run_lifecycle_in_container(
                    docker,
                    &cmd,
                    &id,
                    &workdir,
                    "postAttachCommand",
                    LifecycleMarker::Always,
                    expanded_remote_user.as_deref(),
                    wait_for
                        .as_ref()
                        .is_none_or(|wf| wf.requires(&devcontainer::WaitFor::PostAttachCommand)),
                )?;
            }
        }
    }
    exec_in_container(docker, id, None, &config, &cwd)
}

fn stop(docker: &mut impl Docker, name: Option<String>) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let local_env = local_env_snapshot();
    let (config_path, config) = open_config(&cwd, name.as_deref())?;
    let config_dir = config_path.parent().unwrap_or(cwd.as_path());
    let target = setup::from_config(&config, &cwd, &config_path, config_dir, &local_env);

    if let Existing::Running { id, .. } = lookup_existing(docker, &target, &config_path, &cwd)? {
        match &target {
            ContainerTarget::Single(_) => {
                if !docker.stop(&id)? {
                    return Err(err!("`docker stop` failed"));
                }
            }
            ContainerTarget::Compose(c) => {
                let mut stop_args = c.global_args.clone();
                stop_args.push("stop".to_string());
                stop_args.extend(c.services.iter().cloned());
                if !docker.compose(&stop_args)? {
                    return Err(err!("`docker compose stop` failed"));
                }
            }
        }
    }
    Ok(())
}

fn ps(docker: &mut impl Docker, _name: Option<String>) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let local_env = local_env_snapshot();
    let devcontainer_dir = cwd.join(".devcontainer");
    let configs = discover_configs(&devcontainer_dir);

    if configs.is_empty() {
        return Err(err!(
            "No devcontainer.json found in {}",
            devcontainer_dir.display()
        ));
    }

    let root_config = devcontainer_dir.join("devcontainer.json");

    for config_path in &configs {
        let name = if config_path == &root_config {
            "default".to_string()
        } else {
            config_path
                .parent()
                .and_then(|p| p.file_name())
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "unknown".to_string())
        };

        let content = std::fs::read_to_string(config_path)
            .map_err(|e| err!("Failed to read {}: {e}", config_path.display()))?;
        let config = devcontainer::parse_config(&content)
            .ok_or_else(|| err!("Failed to parse {}", config_path.display()))?;

        let config_dir = config_path.parent().unwrap_or(cwd.as_path());
        let target = setup::from_config(&config, &cwd, config_path, config_dir, &local_env);

        match lookup_existing(docker, &target, config_path, &cwd)? {
            Existing::Running { id, .. } => {
                let short_id = &id[..id.len().min(12)];
                println!("{name}\trunning\t{short_id}");
            }
            Existing::Stopped { .. } => {
                println!("{name}\tstopped");
            }
            Existing::None => {
                println!("{name}\tnone");
            }
        }
    }

    Ok(())
}

fn down(docker: &mut impl Docker, name: Option<String>) -> Result<()> {
    let cwd = std::env::current_dir()?;
    let local_env = local_env_snapshot();
    let (config_path, config) = open_config(&cwd, name.as_deref())?;
    let config_dir = config_path.parent().unwrap_or(cwd.as_path());
    let target = setup::from_config(&config, &cwd, &config_path, config_dir, &local_env);

    match &target {
        ContainerTarget::Single(_) => match lookup_existing(docker, &target, &config_path, &cwd)? {
            Existing::Running { id, .. } | Existing::Stopped { id, .. } => {
                if !docker.remove(&id)? {
                    return Err(err!("`docker rm` failed"));
                }
            }
            Existing::None => {}
        },
        ContainerTarget::Compose(c) => {
            let mut down_args = c.global_args.clone();
            down_args.push("down".to_string());
            if !docker.compose(&down_args)? {
                return Err(err!("`docker compose down` failed"));
            }
        }
    }
    Ok(())
}

fn exec_in_container(
    d: &mut impl Docker,
    id: String,
    found_container: Option<docker::Container>,
    config: &devcontainer::DevcontainerConfig,
    cwd: &std::path::Path,
) -> Result<()> {
    let local_env = local_env_snapshot();
    let container_workspace_folder = config.workspace_folder(cwd, &local_env);
    let container_env: std::collections::HashMap<String, String> = d
        .exec_capture(&[id.as_str(), "printenv"].map(String::from))
        .ok()
        .filter(|o| o.success)
        .map(|o| {
            o.stdout
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
    let remote_user_from_config = config.common().remote_user.as_deref().map(|u| {
        devcontainer::expand_variables(
            u,
            cwd,
            &container_workspace_folder,
            &container_env,
            &local_env,
        )
    });
    let remote_user_from_container = if let Some(ref c) = found_container {
        c.remote_user.clone()
    } else {
        d.inspect_format(&id, "{{index .Config.Labels \"devcontainer.metadata\"}}")
            .ok()
            .and_then(|o| docker::parse_remote_user_from_metadata(o.stdout.trim()))
            .or_else(|| {
                d.inspect_format(&id, "{{.Config.User}}")
                    .ok()
                    .and_then(|o| {
                        let user = o.stdout.trim().to_string();
                        if user.is_empty() { None } else { Some(user) }
                    })
            })
    };
    let remote_user = remote_user_from_config.or(remote_user_from_container);
    let shell = d
        .exec_capture(&[id.as_str(), "printenv", "SHELL"].map(String::from))
        .ok()
        .and_then(|o| {
            let s = o.stdout.trim().to_string();
            if s.is_empty() { None } else { Some(s) }
        })
        .or_else(|| {
            remote_user.as_deref().and_then(|user| {
                d.exec_capture(&[id.as_str(), "getent", "passwd", user].map(String::from))
                    .ok()
                    .and_then(|o| devcontainer::parse_shell_from_passwd(o.stdout.trim()))
            })
        })
        .unwrap_or_else(|| "/bin/sh".to_string());
    let remote_env = config.common().remote_env.as_ref();
    let mut exec_args = vec!["-it".to_string()];
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
                &local_env,
            );
            exec_args.extend(["--env".to_string(), format!("{}={}", key, expanded)]);
        }
    }
    exec_args.extend([id, shell]);
    if !d.exec_interactive(&exec_args)? {
        return Err(err!("`docker exec` failed"));
    }
    Ok(())
}

fn download_features(
    reg: &mut impl Registry,
    host: &mut impl Host,
    features_map: &std::collections::HashMap<String, devcontainer::jsonc::Value>,
    override_order: &[String],
    devcontainer_dir: &std::path::Path,
    cwd: &std::path::Path,
) -> Result<(features::InstallPlan, std::path::PathBuf)> {
    let username = std::env::var("USER").unwrap_or_else(|_| "user".to_string());
    let features_dir = std::env::temp_dir()
        .join(format!("cyyc-{username}"))
        .join("features")
        .join(docker::image_tag(cwd));
    std::fs::create_dir_all(&features_dir)
        .map_err(|e| err!("failed to create features temp dir: {e}"))?;

    let mut sorted: Vec<(&String, &devcontainer::jsonc::Value)> = features_map.iter().collect();
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
            .map_err(|e| err!("failed to create feature dir: {e}"))?;
        match &source {
            features::FeatureSource::Local(path) => {
                if !host.copy_dir(
                    &format!("{}/.", path.display()),
                    &feature_dir.display().to_string(),
                )? {
                    return Err(err!("failed to copy local feature from {}", path.display()));
                }
            }
            features::FeatureSource::Tarball(url) => {
                let tarball = feature_dir.join("feature.tgz");
                if !reg.download(url, &tarball.display().to_string())? {
                    return Err(err!("failed to download feature from {url}"));
                }
                if !reg.unpack_tar(
                    &tarball.display().to_string(),
                    &feature_dir.display().to_string(),
                )? {
                    return Err(err!("failed to extract {}", tarball.display()));
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
                    let output = reg.fetch(&url)?;
                    if !output.success {
                        return Err(err!("failed to fetch OCI token for {registry}/{path}"));
                    }
                    let json = devcontainer::jsonc::parse(&String::from_utf8_lossy(&output.body))
                        .map_err(|e| err!("failed to parse OCI token response: {e}"))?;
                    json.get("token")
                        .and_then(|v| v.as_str())
                        .map(String::from)
                        .ok_or_else(|| err!("OCI token response missing 'token' field"))?
                };
                let manifest_url = format!("https://{registry}/v2/{path}/manifests/{version}");
                let output = reg.fetch_manifest(&manifest_url, &token)?;
                if !output.success {
                    return Err(err!(
                        "failed to fetch OCI manifest for {registry}/{path}:{version}"
                    ));
                }
                let manifest = devcontainer::jsonc::parse(&String::from_utf8_lossy(&output.body))
                    .map_err(|e| err!("failed to parse OCI manifest: {e}"))?;
                let digest = manifest
                    .get("layers")
                    .and_then(|l| l.as_array())
                    .and_then(|l| l.first())
                    .and_then(|l| l.get("digest"))
                    .and_then(|v| v.as_str())
                    .ok_or_else(|| err!("OCI manifest missing layers[0].digest"))?;
                let blob_url = format!("https://{registry}/v2/{path}/blobs/{digest}");
                if !reg.download_blob(&blob_url, &token, &tarball.display().to_string())? {
                    return Err(err!("failed to download OCI blob for {registry}/{path}"));
                }
                if !reg.unpack_tar(
                    &tarball.display().to_string(),
                    &feature_dir.display().to_string(),
                )? {
                    return Err(err!("failed to extract {}", tarball.display()));
                }
            }
        }
        let manifest_content =
            std::fs::read_to_string(feature_dir.join("devcontainer-feature.json"))
                .map_err(|e| err!("devcontainer-feature.json not found in feature {id}: {e}"))?;
        let manifest = features::FeatureManifest::parse(&manifest_content)?;
        resolved.push(features::Feature {
            short_id: manifest.id,
            dir: feature_dir,
            options: (*options).clone(),
            installs_after: manifest.installs_after,
            container_env: manifest.container_env,
            privileged: manifest.privileged,
            init: manifest.init,
            cap_add: manifest.cap_add,
            security_opt: manifest.security_opt,
            mounts: manifest.mounts,
            entrypoint: manifest.entrypoint,
            on_create_command: manifest.on_create_command,
            update_content_command: manifest.update_content_command,
            post_create_command: manifest.post_create_command,
            post_start_command: manifest.post_start_command,
            post_attach_command: manifest.post_attach_command,
        });
    }

    let plan = features::InstallPlan::new(resolved, override_order)?;
    Ok((plan, features_dir))
}

enum Existing {
    Running {
        id: String,
        meta: Option<docker::Container>,
    },
    Stopped {
        id: String,
        meta: Option<docker::Container>,
    },
    None,
}

fn open_config(
    cwd: &std::path::Path,
    name: Option<&str>,
) -> Result<(std::path::PathBuf, devcontainer::DevcontainerConfig)> {
    let devcontainer_dir = cwd.join(".devcontainer");
    let configs = discover_configs(&devcontainer_dir);
    let config_path = select_config(&configs, &devcontainer_dir, name)?;
    let content = std::fs::read_to_string(&config_path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            err!(
                "Dev container config ({}) not found.",
                config_path.display()
            )
        } else {
            err!("Dev container config ({}): {e}", config_path.display())
        }
    })?;
    let config = devcontainer::parse_config(&content).ok_or_else(|| {
        err!(
            "Failed to parse dev container config ({}).",
            config_path.display()
        )
    })?;
    Ok((config_path, config))
}

fn discover_configs(devcontainer_dir: &std::path::Path) -> Vec<std::path::PathBuf> {
    let mut configs = vec![];
    let root = devcontainer_dir.join("devcontainer.json");
    if root.is_file() {
        configs.push(root);
    }
    if let Ok(entries) = std::fs::read_dir(devcontainer_dir) {
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
    configs
}

fn select_config(
    configs: &[std::path::PathBuf],
    devcontainer_dir: &std::path::Path,
    name: Option<&str>,
) -> Result<std::path::PathBuf> {
    match (configs, name) {
        ([], _) => Err(err!(
            "No devcontainer.json found in {}",
            devcontainer_dir.display()
        )),
        ([c], _) => Ok(c.clone()),
        (cs, Some(n)) => {
            let path = devcontainer_dir.join(n).join("devcontainer.json");
            if cs.iter().any(|c| c == &path) {
                Ok(path)
            } else {
                Err(err!("Dev container config ({}) not found.", path.display()))
            }
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
            Err(err!(
                "Multiple devcontainer configs found. Specify a name: {}",
                names.join(", ")
            ))
        }
    }
}

fn lookup_existing(
    docker: &mut impl Docker,
    target: &ContainerTarget,
    config_path: &std::path::Path,
    cwd: &std::path::Path,
) -> Result<Existing> {
    match target {
        ContainerTarget::Compose(c) => {
            if let Some(id) = compose_ps(docker, c, false)? {
                return Ok(Existing::Running { id, meta: None });
            }
            if let Some(id) = compose_ps(docker, c, true)? {
                return Ok(Existing::Stopped { id, meta: None });
            }
            Ok(Existing::None)
        }
        ContainerTarget::Single(_) => {
            if let Some(c) = single_lookup(docker, false, config_path, cwd)? {
                return Ok(Existing::Running {
                    id: c.id.clone(),
                    meta: Some(c),
                });
            }
            if let Some(c) = single_lookup(docker, true, config_path, cwd)? {
                return Ok(Existing::Stopped {
                    id: c.id.clone(),
                    meta: Some(c),
                });
            }
            Ok(Existing::None)
        }
    }
}

fn compose_ps(
    d: &mut impl Docker,
    c: &devcontainer::ComposeArgs,
    all_states: bool,
) -> Result<Option<String>> {
    let output = d.ps_ids(&[c.filter1.clone(), c.filter2.clone()], all_states)?;
    if !output.success {
        return Err(err!(
            "`docker ps` failed with status {}: {}",
            output.status,
            output.stderr.trim()
        ));
    }
    Ok(docker::parse_container_id(&output.stdout))
}

fn single_lookup(
    d: &mut impl Docker,
    all_states: bool,
    config_path: &std::path::Path,
    cwd: &std::path::Path,
) -> Result<Option<docker::Container>> {
    let output = d.ps_ids(&["label=devcontainer.config_file".to_string()], all_states)?;
    if !output.success {
        return Err(err!(
            "`docker ps` failed with status {}: {}",
            output.status,
            output.stderr.trim()
        ));
    }
    let ids = docker::parse_container_ids(&output.stdout);
    if ids.is_empty() {
        return Ok(None);
    }
    let inspect = d.inspect(&ids)?;
    if !inspect.success {
        return Ok(None);
    }
    Ok(docker::find_container(
        inspect.stdout.trim(),
        config_path,
        cwd,
    ))
}

enum LifecycleMarker<'a> {
    Always,
    Once(&'a str),
}

#[allow(clippy::too_many_arguments)]
fn run_lifecycle_in_container(
    d: &mut impl Docker,
    cmd: &LifecycleCmd,
    container_id: &str,
    workdir: &str,
    name: &str,
    marker: LifecycleMarker<'_>,
    remote_user: Option<&str>,
    wait: bool,
) -> Result<()> {
    let user_args: Vec<String> = if let Some(user) = remote_user {
        vec!["--user".to_string(), user.to_string()]
    } else {
        vec![]
    };
    if let LifecycleMarker::Once(epoch) = marker {
        let script = format!(
            "mkdir -p \"$HOME/.devcontainer\" && \
             CONTENT=$(cat \"$HOME/.devcontainer/.{name}Marker\" 2>/dev/null || echo ENOENT) && \
             [ \"${{CONTENT:-{epoch}}}\" != '{epoch}' ] && \
             echo '{epoch}' > \"$HOME/.devcontainer/.{name}Marker\""
        );
        let mut args = user_args.clone();
        args.extend([
            container_id.to_string(),
            "sh".to_string(),
            "-c".to_string(),
            script,
        ]);
        if !d.exec_interactive(&args)? {
            return Ok(());
        }
    }
    let exec_prefix = || {
        let mut a = user_args.clone();
        a.extend([
            "--workdir".to_string(),
            workdir.to_string(),
            container_id.to_string(),
        ]);
        a
    };
    let argvs: Vec<Vec<String>> = match cmd {
        LifecycleCmd::Shell(s) => {
            let mut a = exec_prefix();
            a.extend(["sh".to_string(), "-c".to_string(), s.clone()]);
            vec![a]
        }
        LifecycleCmd::Exec(args) => {
            let mut a = exec_prefix();
            a.extend(args.iter().cloned());
            vec![a]
        }
        LifecycleCmd::Parallel(cmds) => cmds
            .iter()
            .map(|c| {
                let mut a = exec_prefix();
                match c {
                    LifecycleCmd::Shell(s) => {
                        a.extend(["sh".to_string(), "-c".to_string(), s.clone()]);
                    }
                    LifecycleCmd::Exec(args) => {
                        a.extend(args.iter().cloned());
                    }
                    LifecycleCmd::Parallel(_) => {}
                }
                a
            })
            .collect(),
    };
    if wait {
        if !d.exec_group(&argvs, true, name)? {
            return Err(err!("{name} failed"));
        }
    } else {
        d.exec_group(&argvs, false, name)?;
    }
    Ok(())
}

#[cfg(target_os = "linux")]
fn run_uid_docker_build(
    d: &mut impl Docker,
    uid_tag: &str,
    remote_user: &str,
    new_uid: u32,
    new_gid: u32,
    image_user: &str,
    base_image: &str,
) -> Result<()> {
    let username = std::env::var("USER").unwrap_or_else(|_| "user".to_string());
    let uid_dir = std::env::temp_dir()
        .join(format!("cyyc-{username}"))
        .join("uid");
    std::fs::create_dir_all(&uid_dir).map_err(|e| err!("Failed to create uid temp dir: {e}"))?;
    let dockerfile_path = uid_dir.join("updateUID.Dockerfile");
    std::fs::write(&dockerfile_path, crate::uid::UPDATE_UID_DOCKERFILE)
        .map_err(|e| err!("Failed to write updateUID.Dockerfile: {e}"))?;
    let args = [
        "-f",
        &dockerfile_path.display().to_string(),
        "-t",
        uid_tag,
        "--build-arg",
        &format!("BASE_IMAGE={base_image}"),
        "--build-arg",
        &format!("REMOTE_USER={remote_user}"),
        "--build-arg",
        &format!("NEW_UID={new_uid}"),
        "--build-arg",
        &format!("NEW_GID={new_gid}"),
        "--build-arg",
        &format!("IMAGE_USER={image_user}"),
        &uid_dir.display().to_string(),
    ]
    .map(String::from);
    if !d.build_streamed(&args)? {
        return Err(err!("`docker build` for UID update failed"));
    }
    Ok(())
}

fn resolve_lifecycle_user(
    d: &mut impl Docker,
    config: &devcontainer::DevcontainerConfig,
    container_id: &str,
    cwd: &std::path::Path,
) -> Option<String> {
    let local_env = local_env_snapshot();
    let workdir = config.workspace_folder(cwd, &local_env);
    if let Some(u) = config.common().remote_user.as_deref() {
        return Some(devcontainer::expand_variables(
            u,
            cwd,
            &workdir,
            &Default::default(),
            &local_env,
        ));
    }
    d.inspect_format(
        container_id,
        "{{index .Config.Labels \"devcontainer.metadata\"}}",
    )
    .ok()
    .and_then(|o| docker::parse_remote_user_from_metadata(o.stdout.trim()))
    .or_else(|| {
        d.inspect_format(container_id, "{{.Config.User}}")
            .ok()
            .and_then(|o| {
                let user = o.stdout.trim().to_string();
                if user.is_empty() { None } else { Some(user) }
            })
    })
}

fn start_existing(docker: &mut impl Docker, target: &ContainerTarget, id: &str) -> Result<()> {
    match target {
        ContainerTarget::Single(_) => {
            if !docker.start(id)? {
                return Err(err!("`docker start` failed"));
            }
        }
        ContainerTarget::Compose(c) => {
            let mut start_args = c.global_args.clone();
            start_args.push("start".to_string());
            start_args.extend(c.services.iter().cloned());
            if !docker.compose(&start_args)? {
                return Err(err!("`docker compose start` failed"));
            }
        }
    }
    Ok(())
}
