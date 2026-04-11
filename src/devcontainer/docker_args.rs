use super::config::CommonConfig;
use std::path::Path;

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
