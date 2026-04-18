use super::config::DockerComposeConfig;
use std::path::Path;

pub struct ComposeArgs {
    pub project_name: String,
    pub global_args: Vec<String>,
    pub services: Vec<String>,
    pub filter1: String,
    pub filter2: String,
    pub override_content: String,
}

pub fn compose_args(
    config: &DockerComposeConfig,
    cwd: &Path,
    devcontainer_dir: &Path,
) -> ComposeArgs {
    let raw = if devcontainer_dir == cwd.join(".devcontainer") {
        format!(
            "{}_devcontainer",
            cwd.file_name().unwrap_or_default().to_string_lossy()
        )
    } else {
        devcontainer_dir
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string()
    };
    let project_name: String = raw
        .to_lowercase()
        .chars()
        .filter(|c| matches!(c, 'a'..='z' | '0'..='9' | '-' | '_'))
        .collect();
    let mut global_args = vec!["--project-name".to_string(), project_name.clone()];
    global_args.extend(config.docker_compose_file.iter().flat_map(|f| {
        [
            "-f".to_string(),
            devcontainer_dir.join(f).display().to_string(),
        ]
    }));
    let services = {
        let mut s = config.run_services.clone().unwrap_or_default();
        if !s.contains(&config.service) {
            s.push(config.service.clone());
        }
        s
    };
    let filter1 = format!("label=com.docker.compose.project={}", project_name);
    let filter2 = format!("label=com.docker.compose.service={}", config.service);
    let script = r#"echo Container started
trap "exit 0" 15
exec "$$@"
while sleep 1 & wait $$!; do :; done"#;
    let script = script.replace('"', r#"\""#).replace('\n', r"\n");
    let user_line = config
        .common
        .container_user
        .as_deref()
        .map(|u| {
            format!(
                "
    user: {}",
                u
            )
        })
        .unwrap_or_default();
    let override_content = format!(
        "\
services:
  '{}':
    entrypoint: [\"/bin/sh\", \"-c\", \"{}\", \"-\"]{}
",
        config.service, script, user_line
    );
    ComposeArgs {
        project_name,
        global_args,
        services,
        filter1,
        filter2,
        override_content,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devcontainer::config::CommonConfig;

    fn compose_config(service: &str) -> DockerComposeConfig {
        DockerComposeConfig {
            docker_compose_file: vec!["docker-compose.yml".to_string()],
            service: service.to_string(),
            workspace_folder: "/workspace".to_string(),
            run_services: None,
            shutdown_action: None,
            common: CommonConfig {
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
            },
        }
    }

    #[test]
    fn when_compose_args_with_devcontainer_dir_then_project_name_has_devcontainer_suffix() {
        let cwd = Path::new("/home/user/myproject");
        let args = compose_args(&compose_config("app"), cwd, &cwd.join(".devcontainer"));
        assert_eq!(args.project_name, "myproject_devcontainer");
    }

    #[test]
    fn when_compose_args_with_non_devcontainer_dir_then_project_name_is_dir_basename() {
        let cwd = Path::new("/home/user/myproject");
        let args = compose_args(
            &compose_config("app"),
            cwd,
            Path::new("/home/user/myproject/compose"),
        );
        assert_eq!(args.project_name, "compose");
    }

    #[test]
    fn when_compose_args_then_global_args_contain_project_name_flag() {
        let cwd = Path::new("/home/user/myproject");
        let args = compose_args(&compose_config("app"), cwd, &cwd.join(".devcontainer"));
        let idx = args
            .global_args
            .iter()
            .position(|a| a == "--project-name")
            .unwrap();
        assert_eq!(args.global_args[idx + 1], "myproject_devcontainer");
    }

    #[test]
    fn when_compose_args_then_global_args_contain_f_flag_with_absolute_path() {
        let cwd = Path::new("/home/user/myproject");
        let args = compose_args(&compose_config("app"), cwd, &cwd.join(".devcontainer"));
        let idx = args.global_args.iter().position(|a| a == "-f").unwrap();
        assert_eq!(
            args.global_args[idx + 1],
            "/home/user/myproject/.devcontainer/docker-compose.yml"
        );
    }

    #[test]
    fn when_compose_args_without_run_services_then_services_contains_service_only() {
        let cwd = Path::new("/home/user/myproject");
        let args = compose_args(&compose_config("app"), cwd, &cwd.join(".devcontainer"));
        assert_eq!(args.services, vec!["app".to_string()]);
    }

    #[test]
    fn when_compose_args_with_run_services_then_services_starts_with_run_services() {
        let cwd = Path::new("/home/user/myproject");
        let mut config = compose_config("app");
        config.run_services = Some(vec!["db".to_string(), "cache".to_string()]);
        let args = compose_args(&config, cwd, &cwd.join(".devcontainer"));
        assert_eq!(
            args.services,
            vec!["db".to_string(), "cache".to_string(), "app".to_string()]
        );
    }

    #[test]
    fn when_compose_args_with_run_services_including_service_then_no_duplicate() {
        let cwd = Path::new("/home/user/myproject");
        let mut config = compose_config("app");
        config.run_services = Some(vec!["db".to_string(), "app".to_string()]);
        let args = compose_args(&config, cwd, &cwd.join(".devcontainer"));
        assert_eq!(args.services, vec!["db".to_string(), "app".to_string()]);
    }

    #[test]
    fn when_compose_args_then_filters_contain_project_and_service_labels() {
        let cwd = Path::new("/home/user/myproject");
        let args = compose_args(&compose_config("app"), cwd, &cwd.join(".devcontainer"));
        assert_eq!(
            args.filter1,
            "label=com.docker.compose.project=myproject_devcontainer"
        );
        assert_eq!(args.filter2, "label=com.docker.compose.service=app");
    }

    #[test]
    fn when_compose_args_then_override_content_contains_keepalive_entrypoint() {
        let cwd = Path::new("/home/user/myproject");
        let args = compose_args(&compose_config("app"), cwd, &cwd.join(".devcontainer"));
        assert!(args.override_content.contains("while sleep 1"));
        assert!(args.override_content.contains("$$@"));
    }

    #[test]
    fn when_compose_args_with_container_user_then_override_content_contains_user() {
        let cwd = Path::new("/home/user/myproject");
        let mut config = compose_config("app");
        config.common.container_user = Some("vscode".to_string());
        let args = compose_args(&config, cwd, &cwd.join(".devcontainer"));
        assert!(args.override_content.contains("user: vscode"));
    }
}
