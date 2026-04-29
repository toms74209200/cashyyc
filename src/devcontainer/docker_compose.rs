use super::config::DockerComposeConfig;
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub struct ComposeArgs {
    pub project_name: String,
    pub global_args: Vec<String>,
    pub services: Vec<String>,
    pub service: String,
    pub filter1: String,
    pub filter2: String,
    pub override_content: String,
}

#[derive(Deserialize)]
pub struct ComposeResolved {
    pub services: HashMap<String, ServiceResolved>,
}

#[derive(Deserialize)]
#[serde(untagged)]
pub enum ServiceResolved {
    Build { build: BuildResolved },
    Image { image: String },
}

#[derive(Deserialize)]
pub struct BuildResolved {
    pub dockerfile: String,
    pub context: String,
}

pub enum FeatureBaseSource {
    Image(String),
    DockerfilePath(PathBuf),
}

impl ServiceResolved {
    pub fn feature_base_source(&self) -> FeatureBaseSource {
        match self {
            Self::Image { image } => FeatureBaseSource::Image(image.clone()),
            Self::Build { build } => {
                FeatureBaseSource::DockerfilePath(Path::new(&build.context).join(&build.dockerfile))
            }
        }
    }
}

pub fn compose_args(
    config: &DockerComposeConfig,
    cwd: &Path,
    devcontainer_dir: &Path,
) -> ComposeArgs {
    let compose_working_dir = config
        .docker_compose_file
        .first()
        .map(|f| {
            let path = devcontainer_dir.join(f);
            let mut parts = vec![];
            for c in path.components() {
                match c {
                    std::path::Component::ParentDir => {
                        parts.pop();
                    }
                    std::path::Component::CurDir => {}
                    c => parts.push(c),
                }
            }
            parts.iter().collect::<PathBuf>()
        })
        .and_then(|p| p.parent().map(|p| p.to_path_buf()))
        .unwrap_or_else(|| devcontainer_dir.to_path_buf());

    let raw = if compose_working_dir == cwd.join(".devcontainer") {
        format!(
            "{}_devcontainer",
            cwd.file_name().unwrap_or_default().to_string_lossy()
        )
    } else {
        compose_working_dir
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
        let mut s = config.run_services.clone();
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
        service: config.service.clone(),
        filter1,
        filter2,
        override_content,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devcontainer::config::CommonConfig;
    use std::collections::HashMap;

    fn compose_config(service: &str) -> DockerComposeConfig {
        DockerComposeConfig {
            docker_compose_file: vec!["docker-compose.yml".to_string()],
            service: service.to_string(),
            workspace_folder: "/workspace".to_string(),
            run_services: vec![],
            shutdown_action: None,
            common: CommonConfig {
                name: None,
                forward_ports: vec![],
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
                mounts: vec![],
                container_env: HashMap::new(),
                container_user: None,
                init: None,
                privileged: None,
                cap_add: vec![],
                security_opt: vec![],
                remote_env: None,
                remote_user: None,
                update_remote_user_uid: None,
                user_env_probe: None,
                features: HashMap::new(),
                override_feature_install_order: vec![],
                host_requirements: None,
                customizations: HashMap::new(),
            },
        }
    }

    #[test]
    fn when_compose_args_with_named_config_and_parent_compose_file_then_project_name_is_workspace_name()
     {
        let cwd = Path::new("/home/user/myproject");
        let mut config = compose_config("app");
        config.docker_compose_file = vec!["../../docker-compose.yml".to_string()];
        let args = compose_args(&config, cwd, &cwd.join(".devcontainer/server"));
        assert_eq!(args.project_name, "myproject");
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
        config.run_services = vec!["db".to_string(), "cache".to_string()];
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
        config.run_services = vec!["db".to_string(), "app".to_string()];
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

    #[test]
    fn when_service_resolved_image_then_feature_base_source_is_image() {
        let svc: ServiceResolved = serde_json::from_str(r#"{"image":"ubuntu:22.04"}"#).unwrap();
        match svc.feature_base_source() {
            FeatureBaseSource::Image(s) => assert_eq!(s, "ubuntu:22.04"),
            FeatureBaseSource::DockerfilePath(_) => panic!("expected Image"),
        }
    }

    #[test]
    fn when_service_resolved_build_then_feature_base_source_is_dockerfile_path() {
        let svc: ServiceResolved = serde_json::from_str(
            r#"{"build":{"dockerfile":"Dockerfile.dev","context":"/abs/ctx"}}"#,
        )
        .unwrap();
        match svc.feature_base_source() {
            FeatureBaseSource::DockerfilePath(p) => {
                assert_eq!(p, PathBuf::from("/abs/ctx/Dockerfile.dev"));
            }
            FeatureBaseSource::Image(_) => panic!("expected DockerfilePath"),
        }
    }

    #[test]
    fn when_service_resolved_has_both_build_and_image_then_build_is_chosen() {
        let svc: ServiceResolved = serde_json::from_str(
            r#"{"image":"ignored:1","build":{"dockerfile":"D","context":"/c"}}"#,
        )
        .unwrap();
        assert!(matches!(
            svc.feature_base_source(),
            FeatureBaseSource::DockerfilePath(_)
        ));
    }

    #[test]
    fn when_compose_resolved_then_services_are_keyed_by_name() {
        let cfg: ComposeResolved =
            serde_json::from_str(r#"{"services":{"app":{"image":"a:1"},"db":{"image":"b:2"}}}"#)
                .unwrap();
        assert!(cfg.services.contains_key("app"));
        assert!(cfg.services.contains_key("db"));
    }

    #[test]
    fn when_service_resolved_has_neither_then_deserialize_fails() {
        let result: Result<ServiceResolved, _> = serde_json::from_str(r#"{}"#);
        assert!(result.is_err());
    }
}
