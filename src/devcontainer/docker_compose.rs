use super::config::DockerComposeConfig;
use super::jsonc::{self, Value};
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

pub struct ComposeResolved {
    pub services: HashMap<String, ServiceResolved>,
}

impl ComposeResolved {
    pub fn parse(json: &str) -> Option<Self> {
        let value = jsonc::parse(json).ok()?.value();
        let project = value.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let members = value.get("services")?.as_object()?;
        let mut services = HashMap::new();
        for (name, svc) in members {
            services.insert(
                name.clone(),
                ServiceResolved::from_value(svc, &format!("{project}-{name}"))?,
            );
        }
        Some(ComposeResolved { services })
    }
}

pub enum ServiceResolved {
    Build { build: BuildResolved },
    Image { image: String },
}

impl ServiceResolved {
    pub fn from_value(value: &Value, default_image: &str) -> Option<Self> {
        let image = value.get("image").and_then(|v| v.as_str());
        let build = value.get("build").and_then(|b| {
            Some(BuildResolved {
                dockerfile: b.get("dockerfile")?.as_str()?.to_string(),
                context: b.get("context")?.as_str()?.to_string(),
                image: image.unwrap_or(default_image).to_string(),
            })
        });
        if let Some(build) = build {
            return Some(ServiceResolved::Build { build });
        }
        image.map(|image| ServiceResolved::Image {
            image: image.to_string(),
        })
    }
}

pub struct BuildResolved {
    pub dockerfile: String,
    pub context: String,
    pub image: String,
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

fn list_block(key: &str, values: &[String]) -> String {
    if values.is_empty() {
        return String::new();
    }
    let items: String = values.iter().map(|v| format!("\n      - {v}")).collect();
    format!("\n    {key}:{items}")
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
    let init_line = match config.common.init {
        Some(true) => "\n    init: true".to_string(),
        _ => String::new(),
    };
    let user_line = config
        .common
        .container_user
        .as_deref()
        .map(|u| format!("\n    user: {u}"))
        .unwrap_or_default();
    let privileged_line = match config.common.privileged {
        Some(true) => "\n    privileged: true".to_string(),
        _ => String::new(),
    };
    let cap_add_block = list_block("cap_add", &config.common.cap_add);
    let security_opt_block = list_block("security_opt", &config.common.security_opt);
    let service = &config.service;
    let override_content = format!(
        "\
services:
  '{service}':
    entrypoint: [\"/bin/sh\", \"-c\", \"{script}\", \"-\"]{init_line}{user_line}{privileged_line}{cap_add_block}{security_opt_block}
"
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
    use random_string::{CharacterType, generate_random_string};
    use std::collections::HashMap;
    use std::fs::File;

    fn random_word() -> String {
        generate_random_string(
            8,
            &[CharacterType::Lowercase, CharacterType::Numeric],
            "",
            &mut File::open("/dev/urandom").unwrap(),
        )
    }

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
                metadata: Default::default(),
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
    fn when_compose_args_with_init_then_override_content_contains_init() {
        let cwd = Path::new("/home/user/myproject");
        let mut config = compose_config("app");
        config.common.init = Some(true);
        let args = compose_args(&config, cwd, &cwd.join(".devcontainer"));
        assert!(args.override_content.contains("\n    init: true"));
    }

    #[test]
    fn when_compose_args_with_init_false_then_override_content_has_no_init() {
        let cwd = Path::new("/home/user/myproject");
        let mut config = compose_config("app");
        config.common.init = Some(false);
        let args = compose_args(&config, cwd, &cwd.join(".devcontainer"));
        assert!(!args.override_content.contains("init:"));
    }

    #[test]
    fn when_compose_args_with_privileged_then_override_content_contains_privileged() {
        let cwd = Path::new("/home/user/myproject");
        let mut config = compose_config("app");
        config.common.privileged = Some(true);
        let args = compose_args(&config, cwd, &cwd.join(".devcontainer"));
        assert!(args.override_content.contains("\n    privileged: true"));
    }

    #[test]
    fn when_compose_args_with_privileged_false_then_override_content_has_no_privileged() {
        let cwd = Path::new("/home/user/myproject");
        let mut config = compose_config("app");
        config.common.privileged = Some(false);
        let args = compose_args(&config, cwd, &cwd.join(".devcontainer"));
        assert!(!args.override_content.contains("privileged:"));
    }

    #[test]
    fn when_compose_args_with_cap_add_then_override_content_contains_cap_add() {
        let cwd = Path::new("/home/user/myproject");
        let mut config = compose_config("app");
        config.common.cap_add = vec!["SYS_PTRACE".to_string(), "NET_ADMIN".to_string()];
        let args = compose_args(&config, cwd, &cwd.join(".devcontainer"));
        assert!(
            args.override_content
                .contains("\n    cap_add:\n      - SYS_PTRACE\n      - NET_ADMIN")
        );
    }

    #[test]
    fn when_compose_args_without_cap_add_then_override_content_has_no_cap_add() {
        let cwd = Path::new("/home/user/myproject");
        let args = compose_args(&compose_config("app"), cwd, &cwd.join(".devcontainer"));
        assert!(!args.override_content.contains("cap_add:"));
    }

    #[test]
    fn when_compose_args_with_security_opt_then_override_content_contains_security_opt() {
        let cwd = Path::new("/home/user/myproject");
        let mut config = compose_config("app");
        config.common.security_opt = vec!["seccomp=unconfined".to_string()];
        let args = compose_args(&config, cwd, &cwd.join(".devcontainer"));
        assert!(
            args.override_content
                .contains("\n    security_opt:\n      - seccomp=unconfined")
        );
    }

    #[test]
    fn when_compose_args_without_security_opt_then_override_content_has_no_security_opt() {
        let cwd = Path::new("/home/user/myproject");
        let args = compose_args(&compose_config("app"), cwd, &cwd.join(".devcontainer"));
        assert!(!args.override_content.contains("security_opt:"));
    }

    fn service_from(json: &str) -> Option<ServiceResolved> {
        ServiceResolved::from_value(&jsonc::parse(json).unwrap().value(), &random_word())
    }

    #[test]
    fn when_service_resolved_image_then_feature_base_source_is_image() {
        let svc = service_from(r#"{"image":"ubuntu:22.04"}"#).unwrap();
        match svc.feature_base_source() {
            FeatureBaseSource::Image(s) => assert_eq!(s, "ubuntu:22.04"),
            FeatureBaseSource::DockerfilePath(_) => panic!("expected Image"),
        }
    }

    #[test]
    fn when_service_resolved_build_then_feature_base_source_is_dockerfile_path() {
        let svc = service_from(r#"{"build":{"dockerfile":"Dockerfile.dev","context":"/abs/ctx"}}"#)
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
        let svc =
            service_from(r#"{"image":"ignored:1","build":{"dockerfile":"D","context":"/c"}}"#)
                .unwrap();
        assert!(matches!(
            svc.feature_base_source(),
            FeatureBaseSource::DockerfilePath(_)
        ));
    }

    #[test]
    fn when_service_resolved_with_incomplete_build_and_image_then_image_is_chosen() {
        let svc = service_from(r#"{"image":"i:1","build":{"dockerfile":"D"}}"#).unwrap();
        assert!(matches!(
            svc.feature_base_source(),
            FeatureBaseSource::Image(_)
        ));
    }

    #[test]
    fn when_compose_resolved_then_services_are_keyed_by_name() {
        let cfg =
            ComposeResolved::parse(r#"{"services":{"app":{"image":"a:1"},"db":{"image":"b:2"}}}"#)
                .unwrap();
        assert!(cfg.services.contains_key("app"));
        assert!(cfg.services.contains_key("db"));
    }

    #[test]
    fn when_service_resolved_has_neither_then_from_value_fails() {
        assert!(service_from(r#"{}"#).is_none());
    }

    #[test]
    fn when_compose_resolved_with_build_service_without_image_then_build_image_is_project_service()
    {
        let (project, service) = (random_word(), random_word());
        let cfg = ComposeResolved::parse(&format!(
            r#"{{"name":"{project}","services":{{"{service}":{{"build":{{"dockerfile":"D","context":"/c"}}}}}}}}"#
        ))
        .unwrap();
        match cfg.services.get(&service) {
            Some(ServiceResolved::Build { build }) => {
                assert_eq!(build.image, format!("{project}-{service}"));
            }
            _ => panic!("expected Build"),
        }
    }

    #[test]
    fn when_compose_resolved_with_build_service_with_image_then_build_image_is_the_image() {
        let (project, service, image) = (random_word(), random_word(), random_word());
        let cfg = ComposeResolved::parse(&format!(
            r#"{{"name":"{project}","services":{{"{service}":{{"image":"{image}","build":{{"dockerfile":"D","context":"/c"}}}}}}}}"#
        ))
        .unwrap();
        match cfg.services.get(&service) {
            Some(ServiceResolved::Build { build }) => assert_eq!(build.image, image),
            _ => panic!("expected Build"),
        }
    }
}
