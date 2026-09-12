use super::jsonc::Value;
use std::collections::HashMap;

#[derive(Debug, PartialEq, Clone)]
pub struct AppPort(pub String);

#[derive(Debug, PartialEq, Clone)]
pub enum UserEnvProbe {
    None,
    LoginInteractiveShell,
    InteractiveShell,
    LoginShell,
}

impl UserEnvProbe {
    fn from_name(name: &str) -> Option<Self> {
        match name {
            "none" => Some(UserEnvProbe::None),
            "loginInteractiveShell" => Some(UserEnvProbe::LoginInteractiveShell),
            "interactiveShell" => Some(UserEnvProbe::InteractiveShell),
            "loginShell" => Some(UserEnvProbe::LoginShell),
            _ => Option::None,
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
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

    fn from_name(name: &str) -> Option<Self> {
        match name {
            "initializeCommand" => Some(WaitFor::InitializeCommand),
            "onCreateCommand" => Some(WaitFor::OnCreateCommand),
            "updateContentCommand" => Some(WaitFor::UpdateContentCommand),
            "postCreateCommand" => Some(WaitFor::PostCreateCommand),
            "postStartCommand" => Some(WaitFor::PostStartCommand),
            "postAttachCommand" => Some(WaitFor::PostAttachCommand),
            _ => None,
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct PortAttributes {
    pub label: Option<String>,
    pub on_auto_forward: Option<String>,
    pub elevate_if_needed: Option<bool>,
}

impl PortAttributes {
    fn from_value(value: &Value) -> Option<Self> {
        value.as_object()?;
        Some(PortAttributes {
            label: opt_string(value, "label")?,
            on_auto_forward: opt_string(value, "onAutoForward")?,
            elevate_if_needed: opt_bool(value, "elevateIfNeeded")?,
        })
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct HostRequirements {
    pub cpus: Option<u32>,
    pub memory: Option<String>,
    pub storage: Option<String>,
    pub gpu: Option<Value>,
}

impl HostRequirements {
    fn from_value(value: &Value) -> Option<Self> {
        value.as_object()?;
        let cpus = match value.get("cpus") {
            None | Some(Value::Null) => None,
            Some(v) => Some(u32::try_from(v.as_u64()?).ok()?),
        };
        Some(HostRequirements {
            cpus,
            memory: opt_string(value, "memory")?,
            storage: opt_string(value, "storage")?,
            gpu: opt_value(value, "gpu"),
        })
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct CommonConfig {
    pub name: Option<String>,
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
    pub mounts: Vec<Value>,
    pub container_env: HashMap<String, String>,
    pub container_user: Option<String>,
    pub init: Option<bool>,
    pub privileged: Option<bool>,
    pub cap_add: Vec<String>,
    pub security_opt: Vec<String>,
    pub remote_env: Option<HashMap<String, Option<String>>>,
    pub remote_user: Option<String>,
    pub update_remote_user_uid: Option<bool>,
    pub user_env_probe: Option<UserEnvProbe>,
    pub features: HashMap<String, Value>,
    pub override_feature_install_order: Vec<String>,
    pub host_requirements: Option<HostRequirements>,
    pub customizations: HashMap<String, Value>,
}

impl CommonConfig {
    pub(super) fn from_value(value: &Value) -> Option<Self> {
        let ports_attributes = match value.get("portsAttributes") {
            None | Some(Value::Null) => None,
            Some(v) => {
                let members = v.as_object()?;
                let mut map = HashMap::new();
                for (k, attr) in members {
                    map.insert(k.clone(), PortAttributes::from_value(attr)?);
                }
                Some(map)
            }
        };
        let other_ports_attributes = match value.get("otherPortsAttributes") {
            None | Some(Value::Null) => None,
            Some(v) => Some(PortAttributes::from_value(v)?),
        };
        let wait_for = match opt_string(value, "waitFor")? {
            None => None,
            Some(s) => Some(WaitFor::from_name(&s)?),
        };
        let user_env_probe = match opt_string(value, "userEnvProbe")? {
            None => None,
            Some(s) => Some(UserEnvProbe::from_name(&s)?),
        };
        let remote_env = match value.get("remoteEnv") {
            None | Some(Value::Null) => None,
            Some(v) => {
                let members = v.as_object()?;
                let mut map = HashMap::new();
                for (k, entry) in members {
                    let entry = match entry {
                        Value::Null => None,
                        Value::String(s) => Some(s.clone()),
                        _ => return None,
                    };
                    map.insert(k.clone(), entry);
                }
                Some(map)
            }
        };
        let host_requirements = match value.get("hostRequirements") {
            None | Some(Value::Null) => None,
            Some(v) => Some(HostRequirements::from_value(v)?),
        };
        Some(CommonConfig {
            name: opt_string(value, "name")?,
            forward_ports: value_vec(value, "forwardPorts")?,
            ports_attributes,
            other_ports_attributes,
            override_command: opt_bool(value, "overrideCommand")?,
            initialize_command: opt_value(value, "initializeCommand"),
            on_create_command: opt_value(value, "onCreateCommand"),
            update_content_command: opt_value(value, "updateContentCommand"),
            post_create_command: opt_value(value, "postCreateCommand"),
            post_start_command: opt_value(value, "postStartCommand"),
            post_attach_command: opt_value(value, "postAttachCommand"),
            wait_for,
            workspace_folder: opt_string(value, "workspaceFolder")?,
            mounts: value_vec(value, "mounts")?,
            container_env: string_map(value, "containerEnv")?,
            container_user: opt_string(value, "containerUser")?,
            init: opt_bool(value, "init")?,
            privileged: opt_bool(value, "privileged")?,
            cap_add: string_vec(value, "capAdd")?,
            security_opt: string_vec(value, "securityOpt")?,
            remote_env,
            remote_user: opt_string(value, "remoteUser")?,
            update_remote_user_uid: opt_bool(value, "updateRemoteUserUID")?,
            user_env_probe,
            features: value_map(value, "features")?,
            override_feature_install_order: string_vec(value, "overrideFeatureInstallOrder")?,
            host_requirements,
            customizations: value_map(value, "customizations")?,
        })
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct BuildConfig {
    pub dockerfile: Option<String>,
    pub context: Option<String>,
    pub target: Option<String>,
    pub args: HashMap<String, String>,
    pub cache_from: Option<Vec<String>>,
    pub options: Vec<String>,
}

impl BuildConfig {
    fn from_value(value: &Value) -> Option<Self> {
        value.as_object()?;
        Some(BuildConfig {
            dockerfile: opt_string(value, "dockerfile")?,
            context: opt_string(value, "context")?,
            target: opt_string(value, "target")?,
            args: string_map(value, "args")?,
            cache_from: opt_string_or_vec(value, "cacheFrom")?,
            options: string_vec(value, "options")?,
        })
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct DockerComposeConfig {
    pub docker_compose_file: Vec<String>,
    pub service: String,
    pub workspace_folder: String,
    pub run_services: Vec<String>,
    pub shutdown_action: Option<String>,
    pub common: CommonConfig,
}

impl DockerComposeConfig {
    pub(super) fn from_value(value: &Value) -> Option<Self> {
        Some(DockerComposeConfig {
            docker_compose_file: opt_string_or_vec(value, "dockerComposeFile")??,
            service: req_string(value, "service")?,
            workspace_folder: req_string(value, "workspaceFolder")?,
            run_services: string_vec(value, "runServices")?,
            shutdown_action: opt_string(value, "shutdownAction")?,
            common: CommonConfig::from_value(value)?,
        })
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct DockerfileConfig {
    pub docker_file: String,
    pub context: Option<String>,
    pub build: Option<BuildConfig>,
    pub app_port: Vec<AppPort>,
    pub run_args: Vec<String>,
    pub workspace_mount: Option<String>,
    pub shutdown_action: Option<String>,
    pub common: CommonConfig,
}

impl DockerfileConfig {
    pub(super) fn from_value(value: &Value) -> Option<Self> {
        let build = match value.get("build") {
            None | Some(Value::Null) => None,
            Some(v) => Some(BuildConfig::from_value(v)?),
        };
        Some(DockerfileConfig {
            docker_file: req_string(value, "dockerFile")?,
            context: opt_string(value, "context")?,
            build,
            app_port: app_port(value)?,
            run_args: string_vec(value, "runArgs")?,
            workspace_mount: opt_string(value, "workspaceMount")?,
            shutdown_action: opt_string(value, "shutdownAction")?,
            common: CommonConfig::from_value(value)?,
        })
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct DockerfileBuildConfig {
    pub build: BuildConfig,
    pub app_port: Vec<AppPort>,
    pub run_args: Vec<String>,
    pub workspace_mount: Option<String>,
    pub shutdown_action: Option<String>,
    pub common: CommonConfig,
}

impl DockerfileBuildConfig {
    pub(super) fn from_value(value: &Value) -> Option<Self> {
        Some(DockerfileBuildConfig {
            build: BuildConfig::from_value(value.get("build")?)?,
            app_port: app_port(value)?,
            run_args: string_vec(value, "runArgs")?,
            workspace_mount: opt_string(value, "workspaceMount")?,
            shutdown_action: opt_string(value, "shutdownAction")?,
            common: CommonConfig::from_value(value)?,
        })
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct ImageConfig {
    pub image: String,
    pub app_port: Vec<AppPort>,
    pub run_args: Vec<String>,
    pub workspace_mount: Option<String>,
    pub shutdown_action: Option<String>,
    pub common: CommonConfig,
}

impl ImageConfig {
    pub(super) fn from_value(value: &Value) -> Option<Self> {
        Some(ImageConfig {
            image: req_string(value, "image")?,
            app_port: app_port(value)?,
            run_args: string_vec(value, "runArgs")?,
            workspace_mount: opt_string(value, "workspaceMount")?,
            shutdown_action: opt_string(value, "shutdownAction")?,
            common: CommonConfig::from_value(value)?,
        })
    }
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

    pub fn workspace_folder(
        &self,
        cwd: &std::path::Path,
        local_env: &HashMap<String, String>,
    ) -> String {
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
        super::variables::expand_variables(&raw, cwd, &default, &Default::default(), local_env)
    }
}

fn opt_string(value: &Value, key: &str) -> Option<Option<String>> {
    match value.get(key) {
        None | Some(Value::Null) => Some(None),
        Some(Value::String(s)) => Some(Some(s.clone())),
        Some(_) => None,
    }
}

fn req_string(value: &Value, key: &str) -> Option<String> {
    value.get(key)?.as_str().map(String::from)
}

fn opt_bool(value: &Value, key: &str) -> Option<Option<bool>> {
    match value.get(key) {
        None | Some(Value::Null) => Some(None),
        Some(Value::Bool(b)) => Some(Some(*b)),
        Some(_) => None,
    }
}

fn opt_value(value: &Value, key: &str) -> Option<Value> {
    value
        .get(key)
        .filter(|v| !matches!(v, Value::Null))
        .cloned()
}

fn string_vec(value: &Value, key: &str) -> Option<Vec<String>> {
    match value.get(key) {
        None => Some(vec![]),
        Some(Value::Array(items)) => items.iter().map(|v| v.as_str().map(String::from)).collect(),
        Some(_) => None,
    }
}

fn value_vec(value: &Value, key: &str) -> Option<Vec<Value>> {
    match value.get(key) {
        None => Some(vec![]),
        Some(Value::Array(items)) => Some(items.to_vec()),
        Some(_) => None,
    }
}

fn string_map(value: &Value, key: &str) -> Option<HashMap<String, String>> {
    match value.get(key) {
        None => Some(HashMap::new()),
        Some(v) => v.to_string_map(),
    }
}

fn value_map(value: &Value, key: &str) -> Option<HashMap<String, Value>> {
    match value.get(key) {
        None => Some(HashMap::new()),
        Some(v) => {
            let members = v.as_object()?;
            Some(
                members
                    .iter()
                    .map(|(k, v)| (k.clone(), v.clone()))
                    .collect(),
            )
        }
    }
}

fn opt_string_or_vec(value: &Value, key: &str) -> Option<Option<Vec<String>>> {
    match value.get(key) {
        None | Some(Value::Null) => Some(None),
        Some(Value::String(s)) => Some(Some(vec![s.clone()])),
        Some(Value::Array(items)) => items
            .iter()
            .map(|v| v.as_str().map(String::from))
            .collect::<Option<Vec<String>>>()
            .map(Some),
        Some(_) => None,
    }
}

fn app_port(value: &Value) -> Option<Vec<AppPort>> {
    fn normalize(v: &Value) -> Option<AppPort> {
        match v {
            Value::String(s) => Some(AppPort(s.clone())),
            Value::Number(_) => {
                let n = v.as_u64()?;
                Some(AppPort(format!("127.0.0.1:{}:{}", n, n)))
            }
            _ => None,
        }
    }
    match value.get("appPort") {
        None | Some(Value::Null) => Some(vec![]),
        Some(Value::Array(items)) => items.iter().map(normalize).collect(),
        Some(v) => normalize(v).map(|p| vec![p]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devcontainer::jsonc;

    fn value(json: &str) -> Value {
        jsonc::parse(json).unwrap().value()
    }

    #[test]
    fn when_app_port_with_number_then_normalizes_to_loopback_mapping() {
        let result = app_port(&value(r#"{"appPort": 8080}"#)).unwrap();
        assert_eq!(result, vec![AppPort("127.0.0.1:8080:8080".to_string())]);
    }

    #[test]
    fn when_app_port_with_string_then_uses_as_is() {
        let result = app_port(&value(r#"{"appPort": "8080:80"}"#)).unwrap();
        assert_eq!(result, vec![AppPort("8080:80".to_string())]);
    }

    #[test]
    fn when_app_port_with_array_of_numbers_then_normalizes_each() {
        let result = app_port(&value(r#"{"appPort": [3000, 4000]}"#)).unwrap();
        assert_eq!(
            result,
            vec![
                AppPort("127.0.0.1:3000:3000".to_string()),
                AppPort("127.0.0.1:4000:4000".to_string()),
            ]
        );
    }

    #[test]
    fn when_app_port_with_array_of_strings_then_uses_each_as_is() {
        let result = app_port(&value(r#"{"appPort": ["3000:3000", "4000:80"]}"#)).unwrap();
        assert_eq!(
            result,
            vec![
                AppPort("3000:3000".to_string()),
                AppPort("4000:80".to_string()),
            ]
        );
    }

    #[test]
    fn when_app_port_with_null_then_returns_empty() {
        let result = app_port(&value(r#"{"appPort": null}"#)).unwrap();
        assert_eq!(result, vec![]);
    }

    #[test]
    fn when_opt_string_or_vec_with_string_then_returns_some_single_element_vec() {
        let result = opt_string_or_vec(&value(r#"{"k": "hello"}"#), "k").unwrap();
        assert_eq!(result, Some(vec!["hello".to_string()]));
    }

    #[test]
    fn when_opt_string_or_vec_with_array_then_returns_some_vec() {
        let result = opt_string_or_vec(&value(r#"{"k": ["hello", "world"]}"#), "k").unwrap();
        assert_eq!(result, Some(vec!["hello".to_string(), "world".to_string()]));
    }

    #[test]
    fn when_opt_string_or_vec_with_null_then_returns_none() {
        let result = opt_string_or_vec(&value(r#"{"k": null}"#), "k").unwrap();
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
            config.workspace_folder(
                std::path::Path::new("/home/user/myproject"),
                &HashMap::new()
            ),
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
            config.workspace_folder(
                std::path::Path::new("/home/user/myproject"),
                &HashMap::new()
            ),
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
            config.workspace_folder(
                std::path::Path::new("/home/user/myproject"),
                &HashMap::new()
            ),
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
            config.workspace_folder(
                std::path::Path::new("/home/user/myproject"),
                &HashMap::new()
            ),
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
            config.workspace_folder(
                std::path::Path::new("/home/user/myproject"),
                &HashMap::new()
            ),
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
            config.workspace_folder(
                std::path::Path::new("/home/user/myproject"),
                &HashMap::new()
            ),
            "/workspace"
        );
    }
}
