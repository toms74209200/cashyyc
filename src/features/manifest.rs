use super::mount::FeatureMount;
use crate::devcontainer::jsonc::{self, Value};
use crate::err;
use crate::error::Result;
use std::collections::HashMap;
use std::path::PathBuf;

pub struct FeatureManifest {
    pub id: String,
    pub installs_after: Vec<String>,
    pub container_env: HashMap<String, String>,
    pub privileged: Option<bool>,
    pub init: Option<bool>,
    pub cap_add: Vec<String>,
    pub security_opt: Vec<String>,
    pub mounts: Vec<FeatureMount>,
    pub entrypoint: Option<String>,
    pub on_create_command: Option<Value>,
    pub update_content_command: Option<Value>,
    pub post_create_command: Option<Value>,
    pub post_start_command: Option<Value>,
    pub post_attach_command: Option<Value>,
}

fn opt_string(value: &Value, key: &str) -> Result<Option<String>, String> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(s)) => Ok(Some(s.clone())),
        Some(_) => Err(format!("invalid type for field `{key}`")),
    }
}

fn opt_bool(value: &Value, key: &str) -> Result<Option<bool>, String> {
    match value.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(b)) => Ok(Some(*b)),
        Some(_) => Err(format!("invalid type for field `{key}`")),
    }
}

fn string_vec(value: &Value, key: &str) -> Result<Vec<String>, String> {
    match value.get(key) {
        None => Ok(vec![]),
        Some(Value::Array(items)) => items
            .iter()
            .map(|v| {
                v.as_str()
                    .map(String::from)
                    .ok_or_else(|| format!("invalid type for field `{key}`"))
            })
            .collect(),
        Some(_) => Err(format!("invalid type for field `{key}`")),
    }
}

fn opt_value(value: &Value, key: &str) -> Option<Value> {
    value
        .get(key)
        .filter(|v| !matches!(v, Value::Null))
        .cloned()
}

fn string_map(value: &Value, key: &str) -> Result<HashMap<String, String>, String> {
    match value.get(key) {
        None => Ok(HashMap::new()),
        Some(v) => v
            .to_string_map()
            .ok_or_else(|| format!("invalid type for field `{key}`")),
    }
}

impl FeatureManifest {
    pub fn parse(content: &str) -> Result<Self> {
        Self::from_value_content(content)
            .map_err(|e| err!("failed to parse devcontainer-feature.json: {e}"))
    }

    fn from_value_content(content: &str) -> Result<Self, String> {
        let value = jsonc::parse(content).map_err(|e| e.to_string())?;
        let id = match value.get("id") {
            Some(Value::String(s)) => s.clone(),
            Some(_) => return Err("invalid type for field `id`".to_string()),
            None => return Err("missing field `id`".to_string()),
        };
        let mounts = match value.get("mounts") {
            None => vec![],
            Some(Value::Array(items)) => items
                .iter()
                .map(|v| {
                    FeatureMount::from_value(v)
                        .ok_or_else(|| "invalid type for field `mounts`".to_string())
                })
                .collect::<Result<_, _>>()?,
            Some(_) => return Err("invalid type for field `mounts`".to_string()),
        };
        Ok(FeatureManifest {
            id,
            installs_after: string_vec(&value, "installsAfter")?,
            container_env: string_map(&value, "containerEnv")?,
            privileged: opt_bool(&value, "privileged")?,
            init: opt_bool(&value, "init")?,
            cap_add: string_vec(&value, "capAdd")?,
            security_opt: string_vec(&value, "securityOpt")?,
            mounts,
            entrypoint: opt_string(&value, "entrypoint")?,
            on_create_command: opt_value(&value, "onCreateCommand"),
            update_content_command: opt_value(&value, "updateContentCommand"),
            post_create_command: opt_value(&value, "postCreateCommand"),
            post_start_command: opt_value(&value, "postStartCommand"),
            post_attach_command: opt_value(&value, "postAttachCommand"),
        })
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
    pub security_opt: Vec<String>,
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
    fn when_parse_with_security_opt_then_options_are_parsed() {
        let opt = random_name();
        let content = format!(r#"{{"id":"f","securityOpt":["{opt}"]}}"#);
        let m = FeatureManifest::parse(&content).unwrap();
        assert_eq!(m.security_opt, vec![opt]);
    }

    #[test]
    fn when_parse_without_security_opt_then_security_opt_is_empty() {
        let m = FeatureManifest::parse(r#"{"id":"f"}"#).unwrap();
        assert!(m.security_opt.is_empty());
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
        assert_eq!(m.post_create_command, Some(Value::String(cmd)));
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
