use super::config::{BuildConfig, CommonConfig, DockerfileConfig};
use std::path::Path;

pub fn normalize_dockerfile_config(config: &DockerfileConfig) -> BuildConfig {
    BuildConfig {
        dockerfile: config
            .build
            .as_ref()
            .and_then(|b| b.dockerfile.clone())
            .or_else(|| Some(config.docker_file.clone())),
        context: config
            .build
            .as_ref()
            .and_then(|b| b.context.clone())
            .or_else(|| config.context.clone()),
        target: config.build.as_ref().and_then(|b| b.target.clone()),
        args: config.build.as_ref().and_then(|b| b.args.clone()),
        cache_from: config.build.as_ref().and_then(|b| b.cache_from.clone()),
        options: config.build.as_ref().and_then(|b| b.options.clone()),
    }
}

pub fn container_build_args(
    build: &BuildConfig,
    devcontainer_dir: &Path,
    tag: &str,
) -> Vec<String> {
    let dockerfile = build.dockerfile.as_deref().unwrap_or("Dockerfile");
    let context = build.context.as_deref().unwrap_or(".");

    let dockerfile_abs = devcontainer_dir.join(dockerfile);
    let context_abs = devcontainer_dir.join(context);

    let mut args = vec![
        "-t".to_string(),
        tag.to_string(),
        "-f".to_string(),
        dockerfile_abs.display().to_string(),
    ];

    if let Some(target) = &build.target {
        args.extend(["--target".to_string(), target.clone()]);
    }
    if let Some(build_args) = &build.args {
        for (k, v) in build_args {
            args.extend(["--build-arg".to_string(), format!("{}={}", k, v)]);
        }
    }
    if let Some(cache_from) = &build.cache_from {
        for c in cache_from {
            args.extend(["--cache-from".to_string(), c.clone()]);
        }
    }
    if let Some(options) = &build.options {
        args.extend(options.iter().cloned());
    }

    args.push(context_abs.display().to_string());
    args
}

const CONTAINER_LOOP_SCRIPT: &str =
    "echo Container started\ntrap \"exit 0\" 15\nexec \"$@\"\nwhile sleep 1 & wait $!; do :; done";

pub fn container_start_args(
    override_command: Option<bool>,
    image_entrypoint: &[String],
    image_cmd: &[String],
) -> Vec<String> {
    let mut args = vec![
        "-c".to_string(),
        CONTAINER_LOOP_SCRIPT.to_string(),
        "-".to_string(),
    ];
    if override_command == Some(false) {
        args.extend_from_slice(image_entrypoint);
        args.extend_from_slice(image_cmd);
    }
    args
}

pub fn container_run_options(
    common: &CommonConfig,
    run_args: Option<&[String]>,
    workspace_mount: Option<&str>,
    local_folder: &Path,
) -> Vec<String> {
    let mut args = vec![
        "-d".to_string(),
        "--label".to_string(),
        format!("devcontainer.local_folder={}", local_folder.display()),
    ];

    let workspace_folder = common.workspace_folder.clone().unwrap_or_else(|| {
        format!(
            "/workspaces/{}",
            local_folder
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
        )
    });

    let mount = workspace_mount.map(|s| s.to_string()).unwrap_or_else(|| {
        format!(
            "type=bind,source={},target={}",
            local_folder.display(),
            workspace_folder
        )
    });
    args.extend(["--mount".to_string(), mount]);
    args.extend(["-w".to_string(), workspace_folder]);

    if let Some(mounts) = &common.mounts {
        for mount in mounts {
            if let Some(m) = mount.as_str() {
                args.extend(["--mount".to_string(), m.to_string()]);
            }
        }
    }

    if let Some(env) = &common.container_env {
        for (key, value) in env {
            args.extend(["--env".to_string(), format!("{}={}", key, value)]);
        }
    }

    if let Some(user) = &common.container_user {
        args.extend(["--user".to_string(), user.clone()]);
    }

    if common.init == Some(true) {
        args.push("--init".to_string());
    }

    if common.privileged == Some(true) {
        args.push("--privileged".to_string());
    }

    if let Some(caps) = &common.cap_add {
        for cap in caps {
            args.extend(["--cap-add".to_string(), cap.clone()]);
        }
    }

    if let Some(opts) = &common.security_opt {
        for opt in opts {
            args.extend(["--security-opt".to_string(), opt.clone()]);
        }
    }

    if let Some(extra) = run_args {
        args.extend(extra.iter().cloned());
    }

    args
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn empty_common() -> CommonConfig {
        CommonConfig {
            name: None,
            forward_ports: None,
            ports_attributes: None,
            other_ports_attributes: None,
            override_command: None,
            initialize_command: None,
            on_create_command: None,
            update_content_command: None,
            post_create_command: None,
            post_start_command: None,
            post_attach_command: None,
            wait_for: None,
            workspace_folder: None,
            mounts: None,
            container_env: None,
            container_user: None,
            init: None,
            privileged: None,
            cap_add: None,
            security_opt: None,
            remote_env: None,
            remote_user: None,
            update_remote_user_uid: None,
            user_env_probe: None,
            features: None,
            override_feature_install_order: None,
            host_requirements: None,
            customizations: None,
        }
    }

    #[test]
    fn when_container_run_options_then_includes_detach_flag() {
        let args = container_run_options(&empty_common(), None, None, Path::new("/project"));
        assert!(args.contains(&"-d".to_string()));
    }

    #[test]
    fn when_container_run_options_then_includes_local_folder_label() {
        let args =
            container_run_options(&empty_common(), None, None, Path::new("/home/user/project"));
        let label_idx = args.iter().position(|a| a == "--label").unwrap();
        assert_eq!(
            args[label_idx + 1],
            "devcontainer.local_folder=/home/user/project"
        );
    }

    #[test]
    fn when_container_run_options_with_no_workspace_folder_then_uses_basename_as_default() {
        let args = container_run_options(
            &empty_common(),
            None,
            None,
            Path::new("/home/user/myproject"),
        );
        let w_idx = args.iter().position(|a| a == "-w").unwrap();
        assert_eq!(args[w_idx + 1], "/workspaces/myproject");
    }

    #[test]
    fn when_container_run_options_with_workspace_folder_then_uses_it_as_workdir() {
        let mut common = empty_common();
        common.workspace_folder = Some("/workspace".to_string());
        let args = container_run_options(&common, None, None, Path::new("/home/user/project"));
        let w_idx = args.iter().position(|a| a == "-w").unwrap();
        assert_eq!(args[w_idx + 1], "/workspace");
    }

    #[test]
    fn when_container_run_options_with_no_workspace_mount_then_uses_bind_mount_to_workspace_folder()
    {
        let args = container_run_options(
            &empty_common(),
            None,
            None,
            Path::new("/home/user/myproject"),
        );
        let mount_idx = args.iter().position(|a| a == "--mount").unwrap();
        assert_eq!(
            args[mount_idx + 1],
            "type=bind,source=/home/user/myproject,target=/workspaces/myproject"
        );
    }

    #[test]
    fn when_container_run_options_with_workspace_mount_then_uses_it() {
        let custom_mount = "source=/custom,target=/workspace,type=bind";
        let args = container_run_options(
            &empty_common(),
            None,
            Some(custom_mount),
            Path::new("/project"),
        );
        let mount_idx = args.iter().position(|a| a == "--mount").unwrap();
        assert_eq!(args[mount_idx + 1], custom_mount);
    }

    #[test]
    fn when_container_run_options_with_container_env_then_includes_env_flags() {
        let mut common = empty_common();
        let mut env = HashMap::new();
        env.insert("RUST_LOG".to_string(), "debug".to_string());
        common.container_env = Some(env);
        let args = container_run_options(&common, None, None, Path::new("/project"));
        let env_idx = args.iter().position(|a| a == "--env").unwrap();
        assert_eq!(args[env_idx + 1], "RUST_LOG=debug");
    }

    #[test]
    fn when_container_run_options_with_container_user_then_includes_user_flag() {
        let mut common = empty_common();
        common.container_user = Some("vscode".to_string());
        let args = container_run_options(&common, None, None, Path::new("/project"));
        let user_idx = args.iter().position(|a| a == "--user").unwrap();
        assert_eq!(args[user_idx + 1], "vscode");
    }

    #[test]
    fn when_container_run_options_with_init_true_then_includes_init_flag() {
        let mut common = empty_common();
        common.init = Some(true);
        let args = container_run_options(&common, None, None, Path::new("/project"));
        assert!(args.contains(&"--init".to_string()));
    }

    #[test]
    fn when_container_run_options_with_init_false_then_excludes_init_flag() {
        let mut common = empty_common();
        common.init = Some(false);
        let args = container_run_options(&common, None, None, Path::new("/project"));
        assert!(!args.contains(&"--init".to_string()));
    }

    #[test]
    fn when_container_run_options_with_privileged_true_then_includes_privileged_flag() {
        let mut common = empty_common();
        common.privileged = Some(true);
        let args = container_run_options(&common, None, None, Path::new("/project"));
        assert!(args.contains(&"--privileged".to_string()));
    }

    #[test]
    fn when_container_run_options_with_cap_add_then_includes_cap_add_flags() {
        let mut common = empty_common();
        common.cap_add = Some(vec!["SYS_PTRACE".to_string()]);
        let args = container_run_options(&common, None, None, Path::new("/project"));
        let cap_idx = args.iter().position(|a| a == "--cap-add").unwrap();
        assert_eq!(args[cap_idx + 1], "SYS_PTRACE");
    }

    #[test]
    fn when_container_run_options_with_security_opt_then_includes_security_opt_flags() {
        let mut common = empty_common();
        common.security_opt = Some(vec!["seccomp=unconfined".to_string()]);
        let args = container_run_options(&common, None, None, Path::new("/project"));
        let opt_idx = args.iter().position(|a| a == "--security-opt").unwrap();
        assert_eq!(args[opt_idx + 1], "seccomp=unconfined");
    }

    #[test]
    fn when_container_run_options_with_run_args_then_includes_them() {
        let extra = vec!["--network=host".to_string()];
        let args =
            container_run_options(&empty_common(), Some(&extra), None, Path::new("/project"));
        assert!(args.contains(&"--network=host".to_string()));
    }

    #[test]
    fn when_container_start_args_with_override_command_unset_then_returns_loop_script() {
        let args = container_start_args(None, &[], &[]);
        assert_eq!(args[0], "-c");
        assert!(args[1].contains("while sleep 1"));
        assert_eq!(args[2], "-");
    }

    #[test]
    fn when_container_start_args_with_override_command_true_then_returns_loop_script() {
        let args = container_start_args(Some(true), &[], &[]);
        assert_eq!(args[0], "-c");
        assert!(args[1].contains("while sleep 1"));
        assert_eq!(args[2], "-");
    }

    #[test]
    fn when_container_start_args_with_override_command_false_then_appends_image_entrypoint_and_cmd()
    {
        let entrypoint = vec!["/entrypoint.sh".to_string()];
        let cmd = vec!["--flag".to_string()];
        let args = container_start_args(Some(false), &entrypoint, &cmd);
        assert_eq!(args[2], "-");
        assert_eq!(args[3], "/entrypoint.sh");
        assert_eq!(args[4], "--flag");
    }

    #[test]
    fn when_container_start_args_with_override_command_true_then_does_not_append_image_cmd() {
        let entrypoint = vec!["/entrypoint.sh".to_string()];
        let cmd = vec!["--flag".to_string()];
        let args = container_start_args(Some(true), &entrypoint, &cmd);
        assert_eq!(args.len(), 3);
    }

    fn empty_dockerfile_config(docker_file: &str) -> DockerfileConfig {
        DockerfileConfig {
            docker_file: docker_file.to_string(),
            context: None,
            build: None,
            app_port: None,
            run_args: None,
            workspace_mount: None,
            shutdown_action: None,
            common: empty_common(),
        }
    }

    fn empty_build_config() -> BuildConfig {
        BuildConfig {
            dockerfile: None,
            context: None,
            target: None,
            args: None,
            cache_from: None,
            options: None,
        }
    }

    #[test]
    fn when_normalize_dockerfile_config_with_only_docker_file_then_uses_it() {
        let config = empty_dockerfile_config("Dockerfile.dev");
        let result = normalize_dockerfile_config(&config);
        assert_eq!(result.dockerfile, Some("Dockerfile.dev".to_string()));
    }

    #[test]
    fn when_normalize_dockerfile_config_with_top_level_context_then_uses_it() {
        let mut config = empty_dockerfile_config("Dockerfile");
        config.context = Some("..".to_string());
        let result = normalize_dockerfile_config(&config);
        assert_eq!(result.context, Some("..".to_string()));
    }

    #[test]
    fn when_normalize_dockerfile_config_with_build_dockerfile_then_uses_build_dockerfile() {
        let mut config = empty_dockerfile_config("Dockerfile");
        let mut build = empty_build_config();
        build.dockerfile = Some("Dockerfile.prod".to_string());
        config.build = Some(build);
        let result = normalize_dockerfile_config(&config);
        assert_eq!(result.dockerfile, Some("Dockerfile.prod".to_string()));
    }

    #[test]
    fn when_normalize_dockerfile_config_with_build_context_then_uses_build_context() {
        let mut config = empty_dockerfile_config("Dockerfile");
        config.context = Some("..".to_string());
        let mut build = empty_build_config();
        build.context = Some(".".to_string());
        config.build = Some(build);
        let result = normalize_dockerfile_config(&config);
        assert_eq!(result.context, Some(".".to_string()));
    }

    #[test]
    fn when_normalize_dockerfile_config_with_build_but_no_build_dockerfile_then_falls_back_to_top_level()
     {
        let mut config = empty_dockerfile_config("Dockerfile.dev");
        config.build = Some(empty_build_config());
        let result = normalize_dockerfile_config(&config);
        assert_eq!(result.dockerfile, Some("Dockerfile.dev".to_string()));
    }

    #[test]
    fn when_container_build_args_then_includes_tag() {
        let build = empty_build_config();
        let args = container_build_args(&build, Path::new("/project/.devcontainer"), "vsc-myapp");
        assert!(args.contains(&"vsc-myapp".to_string()));
    }

    #[test]
    fn when_container_build_args_then_tag_follows_t_flag() {
        let build = empty_build_config();
        let args = container_build_args(&build, Path::new("/project/.devcontainer"), "vsc-myapp");
        let t_idx = args.iter().position(|a| a == "-t").unwrap();
        assert_eq!(args[t_idx + 1], "vsc-myapp");
    }

    #[test]
    fn when_container_build_args_with_dockerfile_then_resolves_against_devcontainer_dir() {
        let mut build = empty_build_config();
        build.dockerfile = Some("Dockerfile".to_string());
        let args = container_build_args(&build, Path::new("/project/.devcontainer"), "vsc-myapp");
        let f_idx = args.iter().position(|a| a == "-f").unwrap();
        assert_eq!(args[f_idx + 1], "/project/.devcontainer/Dockerfile");
    }

    #[test]
    fn when_container_build_args_with_no_dockerfile_then_defaults_to_dockerfile() {
        let build = empty_build_config();
        let args = container_build_args(&build, Path::new("/project/.devcontainer"), "vsc-myapp");
        let f_idx = args.iter().position(|a| a == "-f").unwrap();
        assert_eq!(args[f_idx + 1], "/project/.devcontainer/Dockerfile");
    }

    #[test]
    fn when_container_build_args_then_last_arg_is_context() {
        let mut build = empty_build_config();
        build.context = Some("..".to_string());
        let args = container_build_args(&build, Path::new("/project/.devcontainer"), "vsc-myapp");
        assert_eq!(args.last().unwrap(), "/project/.devcontainer/..");
    }

    #[test]
    fn when_container_build_args_with_target_then_includes_target() {
        let mut build = empty_build_config();
        build.target = Some("dev".to_string());
        let args = container_build_args(&build, Path::new("/project/.devcontainer"), "vsc-myapp");
        let idx = args.iter().position(|a| a == "--target").unwrap();
        assert_eq!(args[idx + 1], "dev");
    }

    #[test]
    fn when_container_build_args_with_build_args_then_includes_build_arg_flags() {
        let mut build = empty_build_config();
        let mut map = HashMap::new();
        map.insert("VERSION".to_string(), "1.0".to_string());
        build.args = Some(map);
        let args = container_build_args(&build, Path::new("/project/.devcontainer"), "vsc-myapp");
        let idx = args.iter().position(|a| a == "--build-arg").unwrap();
        assert_eq!(args[idx + 1], "VERSION=1.0");
    }

    #[test]
    fn when_container_build_args_with_cache_from_then_includes_cache_from_flags() {
        let mut build = empty_build_config();
        build.cache_from = Some(vec!["myimage:latest".to_string()]);
        let args = container_build_args(&build, Path::new("/project/.devcontainer"), "vsc-myapp");
        let idx = args.iter().position(|a| a == "--cache-from").unwrap();
        assert_eq!(args[idx + 1], "myimage:latest");
    }

    #[test]
    fn when_container_build_args_with_options_then_includes_them() {
        let mut build = empty_build_config();
        build.options = Some(vec!["--no-cache".to_string()]);
        let args = container_build_args(&build, Path::new("/project/.devcontainer"), "vsc-myapp");
        assert!(args.contains(&"--no-cache".to_string()));
    }

    #[test]
    fn when_container_run_options_with_additional_mounts_then_includes_mount_flags() {
        let mut common = empty_common();
        common.mounts = Some(vec![serde_json::Value::String(
            "source=/host/data,target=/container/data,type=bind".to_string(),
        )]);
        let args = container_run_options(&common, None, None, Path::new("/project"));
        let mount_idx = args.iter().rposition(|a| a == "--mount").unwrap();
        assert_eq!(
            args[mount_idx + 1],
            "source=/host/data,target=/container/data,type=bind"
        );
    }
}
