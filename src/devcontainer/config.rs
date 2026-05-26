use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, PartialEq, Clone)]
pub struct AppPort(pub String);

fn deserialize_app_port<'de, D>(deserializer: D) -> Result<Vec<AppPort>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum PortSpec {
        Number(u64),
        Str(String),
    }

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum SingleOrVec {
        Single(PortSpec),
        Vec(Vec<PortSpec>),
    }

    fn normalize(p: PortSpec) -> AppPort {
        match p {
            PortSpec::Number(n) => AppPort(format!("127.0.0.1:{}:{}", n, n)),
            PortSpec::Str(s) => AppPort(s),
        }
    }

    match Option::<SingleOrVec>::deserialize(deserializer)? {
        None => Ok(vec![]),
        Some(SingleOrVec::Single(p)) => Ok(vec![normalize(p)]),
        Some(SingleOrVec::Vec(arr)) => Ok(arr.into_iter().map(normalize).collect()),
    }
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub enum UserEnvProbe {
    None,
    LoginInteractiveShell,
    InteractiveShell,
    LoginShell,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub enum WaitFor {
    InitializeCommand,
    OnCreateCommand,
    UpdateContentCommand,
    PostCreateCommand,
    PostStartCommand,
    PostAttachCommand,
}

impl WaitFor {
    const CHAIN: &'static [WaitFor] = &[
        WaitFor::InitializeCommand,
        WaitFor::OnCreateCommand,
        WaitFor::UpdateContentCommand,
        WaitFor::PostCreateCommand,
        WaitFor::PostStartCommand,
        WaitFor::PostAttachCommand,
    ];

    pub fn requires(&self, cmd: &WaitFor) -> bool {
        let pos = |x: &WaitFor| Self::CHAIN.iter().position(|c| c == x);
        matches!((pos(cmd), pos(self)), (Some(c), Some(s)) if c <= s)
    }
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct PortAttributes {
    pub label: Option<String>,
    pub on_auto_forward: Option<String>,
    pub elevate_if_needed: Option<bool>,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
pub struct HostRequirements {
    pub cpus: Option<u32>,
    pub memory: Option<String>,
    pub storage: Option<String>,
    pub gpu: Option<Value>,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct CommonConfig {
    pub name: Option<String>,
    #[serde(default)]
    pub forward_ports: Vec<Value>,
    pub ports_attributes: Option<HashMap<String, PortAttributes>>,
    pub other_ports_attributes: Option<PortAttributes>,
    pub override_command: Option<bool>,
    pub initialize_command: Option<Value>,
    pub on_create_command: Option<Value>,
    pub update_content_command: Option<Value>,
    pub post_create_command: Option<Value>,
    pub post_start_command: Option<Value>,
    pub post_attach_command: Option<Value>,
    pub wait_for: Option<WaitFor>,
    pub workspace_folder: Option<String>,
    #[serde(default)]
    pub mounts: Vec<Value>,
    #[serde(default)]
    pub container_env: HashMap<String, String>,
    pub container_user: Option<String>,
    pub init: Option<bool>,
    pub privileged: Option<bool>,
    #[serde(default)]
    pub cap_add: Vec<String>,
    #[serde(default)]
    pub security_opt: Vec<String>,
    pub remote_env: Option<HashMap<String, Option<String>>>,
    pub remote_user: Option<String>,
    #[serde(rename = "updateRemoteUserUID")]
    pub update_remote_user_uid: Option<bool>,
    pub user_env_probe: Option<UserEnvProbe>,
    #[serde(default)]
    pub features: HashMap<String, Value>,
    #[serde(default)]
    pub override_feature_install_order: Vec<String>,
    pub host_requirements: Option<HostRequirements>,
    #[serde(default)]
    pub customizations: HashMap<String, Value>,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BuildConfig {
    pub dockerfile: Option<String>,
    pub context: Option<String>,
    pub target: Option<String>,
    #[serde(default)]
    pub args: HashMap<String, String>,
    #[serde(deserialize_with = "deserialize_optional_string_or_vec", default)]
    pub cache_from: Option<Vec<String>>,
    #[serde(default)]
    pub options: Vec<String>,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DockerComposeConfig {
    #[serde(deserialize_with = "deserialize_string_or_vec")]
    pub docker_compose_file: Vec<String>,
    pub service: String,
    pub workspace_folder: String,
    #[serde(default)]
    pub run_services: Vec<String>,
    pub shutdown_action: Option<String>,
    #[serde(flatten)]
    pub common: CommonConfig,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DockerfileConfig {
    pub docker_file: String,
    pub context: Option<String>,
    pub build: Option<BuildConfig>,
    #[serde(default, deserialize_with = "deserialize_app_port")]
    pub app_port: Vec<AppPort>,
    #[serde(default)]
    pub run_args: Vec<String>,
    pub workspace_mount: Option<String>,
    pub shutdown_action: Option<String>,
    #[serde(flatten)]
    pub common: CommonConfig,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DockerfileBuildConfig {
    pub build: BuildConfig,
    #[serde(default, deserialize_with = "deserialize_app_port")]
    pub app_port: Vec<AppPort>,
    #[serde(default)]
    pub run_args: Vec<String>,
    pub workspace_mount: Option<String>,
    pub shutdown_action: Option<String>,
    #[serde(flatten)]
    pub common: CommonConfig,
}

#[derive(Debug, Deserialize, PartialEq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ImageConfig {
    pub image: String,
    #[serde(default, deserialize_with = "deserialize_app_port")]
    pub app_port: Vec<AppPort>,
    #[serde(default)]
    pub run_args: Vec<String>,
    pub workspace_mount: Option<String>,
    pub shutdown_action: Option<String>,
    #[serde(flatten)]
    pub common: CommonConfig,
}

#[derive(Debug, PartialEq, Clone)]
pub enum DevcontainerConfig {
    DockerCompose(DockerComposeConfig),
    Dockerfile(DockerfileConfig),
    DockerfileBuild(DockerfileBuildConfig),
    Image(ImageConfig),
}

impl DevcontainerConfig {
    pub fn common(&self) -> &CommonConfig {
        match self {
            DevcontainerConfig::Image(c) => &c.common,
            DevcontainerConfig::Dockerfile(c) => &c.common,
            DevcontainerConfig::DockerfileBuild(c) => &c.common,
            DevcontainerConfig::DockerCompose(c) => &c.common,
        }
    }

    pub fn workspace_folder(&self, cwd: &std::path::Path) -> String {
        let default = format!(
            "/workspaces/{}",
            cwd.file_name().unwrap_or_default().to_string_lossy()
        );
        let raw = match self {
            DevcontainerConfig::Image(c) => c.common.workspace_folder.clone(),
            DevcontainerConfig::Dockerfile(c) => c.common.workspace_folder.clone(),
            DevcontainerConfig::DockerfileBuild(c) => c.common.workspace_folder.clone(),
            DevcontainerConfig::DockerCompose(c) => Some(c.workspace_folder.clone()),
        }
        .unwrap_or_else(|| default.clone());
        super::variables::expand_variables(&raw, cwd, &default, &Default::default())
    }
}

fn deserialize_string_or_vec<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringOrVec {
        String(String),
        Vec(Vec<String>),
    }
    match StringOrVec::deserialize(deserializer)? {
        StringOrVec::String(s) => Ok(vec![s]),
        StringOrVec::Vec(v) => Ok(v),
    }
}

fn deserialize_optional_string_or_vec<'de, D>(
    deserializer: D,
) -> Result<Option<Vec<String>>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringOrVec {
        String(String),
        Vec(Vec<String>),
    }
    match Option::<StringOrVec>::deserialize(deserializer)? {
        None => Ok(None),
        Some(StringOrVec::String(s)) => Ok(Some(vec![s])),
        Some(StringOrVec::Vec(v)) => Ok(Some(v)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn when_deserialize_app_port_with_number_then_normalizes_to_loopback_mapping() {
        let result = deserialize_app_port(&mut serde_json::Deserializer::from_str("8080")).unwrap();
        assert_eq!(result, vec![AppPort("127.0.0.1:8080:8080".to_string())]);
    }

    #[test]
    fn when_deserialize_app_port_with_string_then_uses_as_is() {
        let result =
            deserialize_app_port(&mut serde_json::Deserializer::from_str(r#""8080:80""#)).unwrap();
        assert_eq!(result, vec![AppPort("8080:80".to_string())]);
    }

    #[test]
    fn when_deserialize_app_port_with_array_of_numbers_then_normalizes_each() {
        let result =
            deserialize_app_port(&mut serde_json::Deserializer::from_str("[3000, 4000]")).unwrap();
        assert_eq!(
            result,
            vec![
                AppPort("127.0.0.1:3000:3000".to_string()),
                AppPort("127.0.0.1:4000:4000".to_string()),
            ]
        );
    }

    #[test]
    fn when_deserialize_app_port_with_array_of_strings_then_uses_each_as_is() {
        let result = deserialize_app_port(&mut serde_json::Deserializer::from_str(
            r#"["3000:3000", "4000:80"]"#,
        ))
        .unwrap();
        assert_eq!(
            result,
            vec![
                AppPort("3000:3000".to_string()),
                AppPort("4000:80".to_string()),
            ]
        );
    }

    #[test]
    fn when_deserialize_app_port_with_null_then_returns_empty() {
        let result = deserialize_app_port(&mut serde_json::Deserializer::from_str("null")).unwrap();
        assert_eq!(result, vec![]);
    }

    #[test]
    fn when_deserialize_string_or_vec_with_string_then_returns_single_element_vec() {
        let result =
            deserialize_string_or_vec(&mut serde_json::Deserializer::from_str(r#""hello""#))
                .unwrap();
        assert_eq!(result, vec!["hello".to_string()]);
    }

    #[test]
    fn when_deserialize_string_or_vec_with_array_then_returns_vec() {
        let result = deserialize_string_or_vec(&mut serde_json::Deserializer::from_str(
            r#"["hello", "world"]"#,
        ))
        .unwrap();
        assert_eq!(result, vec!["hello".to_string(), "world".to_string()]);
    }

    #[test]
    fn when_deserialize_optional_string_or_vec_with_string_then_returns_some_single_element_vec() {
        let result = deserialize_optional_string_or_vec(&mut serde_json::Deserializer::from_str(
            r#""hello""#,
        ))
        .unwrap();
        assert_eq!(result, Some(vec!["hello".to_string()]));
    }

    #[test]
    fn when_deserialize_optional_string_or_vec_with_array_then_returns_some_vec() {
        let result = deserialize_optional_string_or_vec(&mut serde_json::Deserializer::from_str(
            r#"["hello", "world"]"#,
        ))
        .unwrap();
        assert_eq!(result, Some(vec!["hello".to_string(), "world".to_string()]));
    }

    #[test]
    fn when_deserialize_optional_string_or_vec_with_null_then_returns_none() {
        let result =
            deserialize_optional_string_or_vec(&mut serde_json::Deserializer::from_str("null"))
                .unwrap();
        assert_eq!(result, None);
    }

    fn empty_common() -> CommonConfig {
        CommonConfig {
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
            container_env: Default::default(),
            container_user: None,
            init: None,
            privileged: None,
            cap_add: vec![],
            security_opt: vec![],
            remote_env: None,
            remote_user: None,
            update_remote_user_uid: None,
            user_env_probe: None,
            features: Default::default(),
            override_feature_install_order: vec![],
            host_requirements: None,
            customizations: Default::default(),
        }
    }

    fn empty_build() -> BuildConfig {
        BuildConfig {
            dockerfile: None,
            context: None,
            target: None,
            args: Default::default(),
            cache_from: None,
            options: vec![],
        }
    }

    #[test]
    fn when_common_for_image_then_returns_image_common() {
        let config = DevcontainerConfig::Image(ImageConfig {
            image: "img".to_string(),
            app_port: vec![],
            run_args: vec![],
            workspace_mount: None,
            shutdown_action: None,
            common: CommonConfig {
                name: Some("my-image".to_string()),
                ..empty_common()
            },
        });
        assert_eq!(config.common().name, Some("my-image".to_string()));
    }

    #[test]
    fn when_common_for_dockerfile_then_returns_dockerfile_common() {
        let config = DevcontainerConfig::Dockerfile(DockerfileConfig {
            docker_file: "Dockerfile".to_string(),
            context: None,
            build: None,
            app_port: vec![],
            run_args: vec![],
            workspace_mount: None,
            shutdown_action: None,
            common: CommonConfig {
                name: Some("my-dockerfile".to_string()),
                ..empty_common()
            },
        });
        assert_eq!(config.common().name, Some("my-dockerfile".to_string()));
    }

    #[test]
    fn when_common_for_dockerfile_build_then_returns_dockerfile_build_common() {
        let config = DevcontainerConfig::DockerfileBuild(DockerfileBuildConfig {
            build: empty_build(),
            app_port: vec![],
            run_args: vec![],
            workspace_mount: None,
            shutdown_action: None,
            common: CommonConfig {
                name: Some("my-build".to_string()),
                ..empty_common()
            },
        });
        assert_eq!(config.common().name, Some("my-build".to_string()));
    }

    #[test]
    fn when_common_for_compose_then_returns_compose_common() {
        let config = DevcontainerConfig::DockerCompose(DockerComposeConfig {
            docker_compose_file: vec!["docker-compose.yml".to_string()],
            service: "app".to_string(),
            workspace_folder: "/workspace".to_string(),
            run_services: vec![],
            shutdown_action: None,
            common: CommonConfig {
                name: Some("my-compose".to_string()),
                ..empty_common()
            },
        });
        assert_eq!(config.common().name, Some("my-compose".to_string()));
    }

    #[test]
    fn when_workspace_folder_with_explicit_path_then_returns_it() {
        let config = DevcontainerConfig::Image(ImageConfig {
            image: "img".to_string(),
            app_port: vec![],
            run_args: vec![],
            workspace_mount: None,
            shutdown_action: None,
            common: CommonConfig {
                workspace_folder: Some("/workspace".to_string()),
                ..empty_common()
            },
        });
        assert_eq!(
            config.workspace_folder(std::path::Path::new("/home/user/myproject")),
            "/workspace"
        );
    }

    #[test]
    fn when_workspace_folder_without_explicit_path_then_uses_cwd_basename() {
        let config = DevcontainerConfig::Image(ImageConfig {
            image: "img".to_string(),
            app_port: vec![],
            run_args: vec![],
            workspace_mount: None,
            shutdown_action: None,
            common: empty_common(),
        });
        assert_eq!(
            config.workspace_folder(std::path::Path::new("/home/user/myproject")),
            "/workspaces/myproject"
        );
    }

    #[test]
    fn when_workspace_folder_for_dockerfile_variant_with_explicit_path_then_returns_it() {
        let config = DevcontainerConfig::Dockerfile(DockerfileConfig {
            docker_file: "Dockerfile".to_string(),
            context: None,
            build: None,
            app_port: vec![],
            run_args: vec![],
            workspace_mount: None,
            shutdown_action: None,
            common: CommonConfig {
                workspace_folder: Some("/workspace".to_string()),
                ..empty_common()
            },
        });
        assert_eq!(
            config.workspace_folder(std::path::Path::new("/home/user/myproject")),
            "/workspace"
        );
    }

    #[test]
    fn when_workspace_folder_for_dockerfile_build_variant_with_explicit_path_then_returns_it() {
        let config = DevcontainerConfig::DockerfileBuild(DockerfileBuildConfig {
            build: empty_build(),
            app_port: vec![],
            run_args: vec![],
            workspace_mount: None,
            shutdown_action: None,
            common: CommonConfig {
                workspace_folder: Some("/workspace".to_string()),
                ..empty_common()
            },
        });
        assert_eq!(
            config.workspace_folder(std::path::Path::new("/home/user/myproject")),
            "/workspace"
        );
    }

    #[test]
    fn when_workspace_folder_with_local_workspace_folder_basename_variable_then_expands_it() {
        let config = DevcontainerConfig::Image(ImageConfig {
            image: "img".to_string(),
            app_port: vec![],
            run_args: vec![],
            workspace_mount: None,
            shutdown_action: None,
            common: CommonConfig {
                workspace_folder: Some("/workspaces/${localWorkspaceFolderBasename}".to_string()),
                ..empty_common()
            },
        });
        assert_eq!(
            config.workspace_folder(std::path::Path::new("/home/user/myproject")),
            "/workspaces/myproject"
        );
    }

    #[test]
    fn when_compose_workspace_folder_then_returns_compose_field() {
        let config = DevcontainerConfig::DockerCompose(DockerComposeConfig {
            docker_compose_file: vec!["docker-compose.yml".to_string()],
            service: "app".to_string(),
            workspace_folder: "/workspace".to_string(),
            run_services: vec![],
            shutdown_action: None,
            common: empty_common(),
        });
        assert_eq!(
            config.workspace_folder(std::path::Path::new("/home/user/myproject")),
            "/workspace"
        );
    }
}
