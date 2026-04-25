use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;

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
    pub app_port: Option<Value>,
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
    pub app_port: Option<Value>,
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
    pub app_port: Option<Value>,
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
}
