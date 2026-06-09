use super::mount::FeatureMount;
use anyhow::{Result, anyhow};
use serde::Deserialize;
use serde_json::Value;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Deserialize)]
pub struct FeatureManifest {
    pub id: String,
    #[serde(rename = "installsAfter", default)]
    pub installs_after: Vec<String>,
    #[serde(rename = "containerEnv", default)]
    pub container_env: HashMap<String, String>,
    #[serde(default)]
    pub privileged: Option<bool>,
    #[serde(default)]
    pub init: Option<bool>,
    #[serde(rename = "capAdd", default)]
    pub cap_add: Vec<String>,
    #[serde(default)]
    pub mounts: Vec<FeatureMount>,
    #[serde(default)]
    pub entrypoint: Option<String>,
    #[serde(rename = "onCreateCommand", default)]
    pub on_create_command: Option<Value>,
    #[serde(rename = "updateContentCommand", default)]
    pub update_content_command: Option<Value>,
    #[serde(rename = "postCreateCommand", default)]
    pub post_create_command: Option<Value>,
    #[serde(rename = "postStartCommand", default)]
    pub post_start_command: Option<Value>,
    #[serde(rename = "postAttachCommand", default)]
    pub post_attach_command: Option<Value>,
}

impl FeatureManifest {
    pub fn parse(content: &str) -> Result<Self> {
        serde_json::from_str(content)
            .map_err(|e| anyhow!("failed to parse devcontainer-feature.json: {e}"))
    }
}

pub struct Feature {
    pub short_id: String,
    pub dir: PathBuf,
    pub options: Value,
    pub installs_after: Vec<String>,
    pub container_env: HashMap<String, String>,
    pub privileged: Option<bool>,
    pub init: Option<bool>,
    pub cap_add: Vec<String>,
    pub mounts: Vec<FeatureMount>,
    pub entrypoint: Option<String>,
    pub on_create_command: Option<Value>,
    pub update_content_command: Option<Value>,
    pub post_create_command: Option<Value>,
    pub post_start_command: Option<Value>,
    pub post_attach_command: Option<Value>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use random_string::{CharacterType, generate_random_string};
    use std::fs::File;

    fn urandom() -> File {
        File::open("/dev/urandom").unwrap()
    }

    fn random_name() -> String {
        generate_random_string(8, &[CharacterType::Lowercase], "", &mut urandom())
    }

    #[test]
    fn when_parse_with_installs_after_then_ids_are_parsed() {
        let dep = random_name();
        let content = format!(r#"{{"id":"git","installsAfter":["{dep}"]}}"#);
        let m = FeatureManifest::parse(&content).unwrap();
        assert_eq!(m.installs_after, vec![dep]);
    }

    #[test]
    fn when_parse_with_privileged_true_then_privileged_is_some_true() {
        let m = FeatureManifest::parse(r#"{"id":"f","privileged":true}"#).unwrap();
        assert_eq!(m.privileged, Some(true));
    }

    #[test]
    fn when_parse_without_privileged_then_privileged_is_none() {
        let m = FeatureManifest::parse(r#"{"id":"f"}"#).unwrap();
        assert_eq!(m.privileged, None);
    }

    #[test]
    fn when_parse_with_init_true_then_init_is_some_true() {
        let m = FeatureManifest::parse(r#"{"id":"f","init":true}"#).unwrap();
        assert_eq!(m.init, Some(true));
    }

    #[test]
    fn when_parse_without_init_then_init_is_none() {
        let m = FeatureManifest::parse(r#"{"id":"f"}"#).unwrap();
        assert_eq!(m.init, None);
    }

    #[test]
    fn when_parse_with_cap_add_then_capabilities_are_parsed() {
        let cap = random_name();
        let content = format!(r#"{{"id":"f","capAdd":["{cap}"]}}"#);
        let m = FeatureManifest::parse(&content).unwrap();
        assert_eq!(m.cap_add, vec![cap]);
    }

    #[test]
    fn when_parse_without_cap_add_then_cap_add_is_empty() {
        let m = FeatureManifest::parse(r#"{"id":"f"}"#).unwrap();
        assert!(m.cap_add.is_empty());
    }

    #[test]
    fn when_parse_with_mounts_then_mount_fields_are_parsed() {
        let source = format!("/var/run/{}", random_name());
        let target = format!("/var/run/{}", random_name());
        let content = format!(
            r#"{{"id":"f","mounts":[{{"type":"bind","source":"{source}","target":"{target}"}}]}}"#
        );
        let m = FeatureManifest::parse(&content).unwrap();
        assert_eq!(m.mounts.len(), 1);
        assert_eq!(m.mounts[0].mount_type, "bind");
        assert_eq!(m.mounts[0].source.as_deref(), Some(source.as_str()));
        assert_eq!(m.mounts[0].target, target);
    }

    #[test]
    fn when_parse_without_mounts_then_mounts_is_empty() {
        let m = FeatureManifest::parse(r#"{"id":"f"}"#).unwrap();
        assert!(m.mounts.is_empty());
    }

    #[test]
    fn when_parse_with_entrypoint_then_entrypoint_is_some() {
        let ep = format!("/usr/local/share/{}-init.sh", random_name());
        let content = format!(r#"{{"id":"f","entrypoint":"{ep}"}}"#);
        let m = FeatureManifest::parse(&content).unwrap();
        assert_eq!(m.entrypoint, Some(ep));
    }

    #[test]
    fn when_parse_without_entrypoint_then_entrypoint_is_none() {
        let m = FeatureManifest::parse(r#"{"id":"f"}"#).unwrap();
        assert_eq!(m.entrypoint, None);
    }

    #[test]
    fn when_parse_with_invalid_json_then_returns_error() {
        assert!(FeatureManifest::parse("not json").is_err());
    }

    #[test]
    fn when_parse_without_id_field_then_returns_error() {
        assert!(FeatureManifest::parse("{}").is_err());
    }

    #[test]
    fn when_parse_with_non_string_id_then_returns_error() {
        assert!(FeatureManifest::parse(r#"{"id":123}"#).is_err());
    }

    #[test]
    fn when_parse_with_post_create_command_string_then_parsed_as_value() {
        let cmd = random_name();
        let content = format!(r#"{{"id":"f","postCreateCommand":"{cmd}"}}"#);
        let m = FeatureManifest::parse(&content).unwrap();
        assert_eq!(m.post_create_command, Some(serde_json::Value::String(cmd)));
    }

    #[test]
    fn when_parse_without_lifecycle_commands_then_all_are_none() {
        let m = FeatureManifest::parse(r#"{"id":"f"}"#).unwrap();
        assert!(m.on_create_command.is_none());
        assert!(m.update_content_command.is_none());
        assert!(m.post_create_command.is_none());
        assert!(m.post_start_command.is_none());
        assert!(m.post_attach_command.is_none());
    }
}
