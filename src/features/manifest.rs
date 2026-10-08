use crate::devcontainer::jsonc::{self, Value};
use crate::err;
use crate::error::Result;
use std::collections::HashMap;
use std::path::PathBuf;

const METADATA_PROPERTIES: [&str; 12] = [
    "onCreateCommand",
    "updateContentCommand",
    "postCreateCommand",
    "postStartCommand",
    "postAttachCommand",
    "init",
    "privileged",
    "capAdd",
    "securityOpt",
    "entrypoint",
    "mounts",
    "customizations",
];

pub struct FeatureManifest {
    pub id: String,
    pub installs_after: Vec<String>,
    pub container_env: HashMap<String, String>,
    pub metadata: Vec<(String, Value)>,
    pub options: Value,
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

fn string_map(value: &Value, key: &str) -> Result<HashMap<String, String>, String> {
    match value.get(key) {
        None => Ok(HashMap::new()),
        Some(v) => v
            .to_string_map()
            .ok_or_else(|| format!("invalid type for field `{key}`")),
    }
}

impl FeatureManifest {
    pub fn parse(user_feature_id: &str, user_options: &Value, content: &str) -> Result<Self> {
        Self::from_value_content(user_feature_id, user_options, content)
            .map_err(|e| err!("failed to parse devcontainer-feature.json: {e}"))
    }

    fn from_value_content(
        user_feature_id: &str,
        user_options: &Value,
        content: &str,
    ) -> Result<Self, String> {
        let value = jsonc::parse(content).map_err(|e| e.to_string())?.value();
        let id = match value.get("id") {
            Some(Value::String(s)) => s.clone(),
            Some(_) => return Err("invalid type for field `id`".to_string()),
            None => return Err("missing field `id`".to_string()),
        };
        match value.get("mounts") {
            None => {}
            Some(Value::Array(items))
                if items.iter().all(|v| {
                    v.get("type").and_then(Value::as_str).is_some()
                        && v.get("target").and_then(Value::as_str).is_some()
                        && matches!(v.get("source"), None | Some(Value::Null | Value::String(_)))
                }) => {}
            Some(_) => return Err("invalid type for field `mounts`".to_string()),
        }
        let user_options = match user_options {
            Value::Object(members) => members.as_slice(),
            _ => &[],
        };
        let options = match value.get("options") {
            None => Value::Object(user_options.to_vec()),
            Some(Value::Object(options))
                if options
                    .iter()
                    .all(|(_, option)| matches!(option, Value::Object(_))) =>
            {
                Value::Object(
                    options
                        .iter()
                        .filter(|(name, _)| user_options.iter().all(|(k, _)| k != name))
                        .filter_map(|(name, option)| {
                            option.get("default").map(|d| (name.clone(), d.clone()))
                        })
                        .chain(user_options.iter().cloned())
                        .collect(),
                )
            }
            Some(_) => return Err("invalid type for field `options`".to_string()),
        };
        opt_bool(&value, "privileged")?;
        opt_bool(&value, "init")?;
        string_vec(&value, "capAdd")?;
        string_vec(&value, "securityOpt")?;
        opt_string(&value, "entrypoint")?;
        Ok(FeatureManifest {
            id,
            installs_after: string_vec(&value, "installsAfter")?,
            container_env: string_map(&value, "containerEnv")?,
            metadata: std::iter::once((
                "id".to_string(),
                Value::String(user_feature_id.to_string()),
            ))
            .chain(
                METADATA_PROPERTIES
                    .iter()
                    .filter_map(|k| value.get(k).map(|v| (k.to_string(), v.clone()))),
            )
            .collect(),
            options,
        })
    }
}

pub struct Feature {
    pub short_id: String,
    pub metadata: Vec<(String, Value)>,
    pub dir: PathBuf,
    pub options: Value,
    pub installs_after: Vec<String>,
    pub container_env: HashMap<String, String>,
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
        let m = FeatureManifest::parse(&random_name(), &Value::Object(vec![]), &content).unwrap();
        assert_eq!(m.installs_after, vec![dep]);
    }

    #[test]
    fn when_parse_with_non_bool_privileged_then_returns_error() {
        let content = format!(r#"{{"id":"f","privileged":"{}"}}"#, random_name());

        let result = FeatureManifest::parse(&random_name(), &Value::Object(vec![]), &content);

        assert!(result.is_err());
    }

    #[test]
    fn when_parse_with_non_bool_init_then_returns_error() {
        let content = format!(r#"{{"id":"f","init":"{}"}}"#, random_name());

        let result = FeatureManifest::parse(&random_name(), &Value::Object(vec![]), &content);

        assert!(result.is_err());
    }

    #[test]
    fn when_parse_with_non_array_cap_add_then_returns_error() {
        let content = format!(r#"{{"id":"f","capAdd":"{}"}}"#, random_name());

        let result = FeatureManifest::parse(&random_name(), &Value::Object(vec![]), &content);

        assert!(result.is_err());
    }

    #[test]
    fn when_parse_with_non_string_security_opt_then_returns_error() {
        let content = r#"{"id":"f","securityOpt":[1]}"#;

        let result = FeatureManifest::parse(&random_name(), &Value::Object(vec![]), content);

        assert!(result.is_err());
    }

    #[test]
    fn when_parse_with_non_array_mounts_then_returns_error() {
        let content = format!(r#"{{"id":"f","mounts":"{}"}}"#, random_name());

        let result = FeatureManifest::parse(&random_name(), &Value::Object(vec![]), &content);

        assert!(result.is_err());
    }

    #[test]
    fn when_parse_with_mount_without_target_then_returns_error() {
        let content = r#"{"id":"f","mounts":[{"type":"bind"}]}"#;

        let result = FeatureManifest::parse(&random_name(), &Value::Object(vec![]), content);

        assert!(result.is_err());
    }

    #[test]
    fn when_parse_with_mount_without_type_then_returns_error() {
        let content = format!(
            r#"{{"id":"f","mounts":[{{"target":"/{}"}}]}}"#,
            random_name()
        );

        let result = FeatureManifest::parse(&random_name(), &Value::Object(vec![]), &content);

        assert!(result.is_err());
    }

    #[test]
    fn when_parse_with_mount_of_non_string_source_then_returns_error() {
        let content = format!(
            r#"{{"id":"f","mounts":[{{"type":"bind","source":1,"target":"/{}"}}]}}"#,
            random_name()
        );

        let result = FeatureManifest::parse(&random_name(), &Value::Object(vec![]), &content);

        assert!(result.is_err());
    }

    #[test]
    fn when_parse_with_mount_without_source_then_metadata_has_the_mount() {
        let target = format!("/{}", random_name());
        let content = format!(r#"{{"id":"f","mounts":[{{"type":"volume","target":"{target}"}}]}}"#);

        let m = FeatureManifest::parse(&random_name(), &Value::Object(vec![]), &content).unwrap();

        assert_eq!(
            m.metadata.iter().find(|(key, _)| key == "mounts"),
            Some(&(
                "mounts".to_string(),
                jsonc::parse(&format!(r#"[{{"type":"volume","target":"{target}"}}]"#))
                    .unwrap()
                    .value()
            ))
        );
    }

    #[test]
    fn when_parse_with_entrypoint_then_metadata_has_the_entrypoint() {
        let ep = format!("/usr/local/share/{}-init.sh", random_name());
        let content = format!(r#"{{"id":"f","entrypoint":"{ep}"}}"#);
        let m = FeatureManifest::parse(&random_name(), &Value::Object(vec![]), &content).unwrap();
        assert_eq!(
            m.metadata.iter().find(|(key, _)| key == "entrypoint"),
            Some(&("entrypoint".to_string(), Value::String(ep)))
        );
    }

    #[test]
    fn when_parse_with_non_string_entrypoint_then_returns_error() {
        assert!(
            FeatureManifest::parse(
                &random_name(),
                &Value::Object(vec![]),
                r#"{"id":"f","entrypoint":1}"#
            )
            .is_err()
        );
    }

    #[test]
    fn when_parse_without_entrypoint_then_metadata_has_no_entrypoint() {
        let m = FeatureManifest::parse(&random_name(), &Value::Object(vec![]), r#"{"id":"f"}"#)
            .unwrap();
        assert!(!m.metadata.iter().any(|(key, _)| key == "entrypoint"));
    }

    #[test]
    fn when_parse_with_invalid_json_then_returns_error() {
        assert!(
            FeatureManifest::parse(&random_name(), &Value::Object(vec![]), "not json").is_err()
        );
    }

    #[test]
    fn when_parse_without_id_field_then_returns_error() {
        assert!(FeatureManifest::parse(&random_name(), &Value::Object(vec![]), "{}").is_err());
    }

    #[test]
    fn when_parse_with_non_string_id_then_returns_error() {
        assert!(
            FeatureManifest::parse(&random_name(), &Value::Object(vec![]), r#"{"id":123}"#)
                .is_err()
        );
    }

    #[test]
    fn when_parse_then_metadata_has_user_feature_id_and_metadata_properties() {
        let (user_feature_id, name, cmd) = (random_name(), random_name(), random_name());
        let content = format!(
            r#"{{
                "id": "f",
                "version": "1.0.0",
                "name": "{name}",
                "options": {{"version": {{"type": "string", "default": "{name}"}}}},
                "containerEnv": {{"KEY": "{name}"}},
                "installsAfter": ["{name}"],
                "privileged": true,
                "postCreateCommand": "{cmd}"
            }}"#
        );
        let m = FeatureManifest::parse(&user_feature_id, &Value::Object(vec![]), &content).unwrap();
        assert_eq!(
            m.metadata,
            jsonc::parse(&format!(
                r#"{{"id":"{user_feature_id}","postCreateCommand":"{cmd}","privileged":true}}"#
            ))
            .unwrap()
            .value()
            .as_object()
            .unwrap()
        );
    }

    #[test]
    fn when_parse_with_declared_default_and_no_user_value_then_options_have_the_default() {
        let (name, default) = (random_name(), random_name());
        let content = format!(
            r#"{{"id":"f","options":{{"{name}":{{"type":"string","default":"{default}"}}}}}}"#
        );

        let m = FeatureManifest::parse(&random_name(), &Value::Object(vec![]), &content).unwrap();

        assert_eq!(
            m.options,
            jsonc::parse(&format!(r#"{{"{name}":"{default}"}}"#))
                .unwrap()
                .value()
        );
    }

    #[test]
    fn when_parse_with_user_value_for_declared_default_then_options_have_the_user_value() {
        let (name, default, user_value) = (random_name(), random_name(), random_name());
        let content = format!(
            r#"{{"id":"f","options":{{"{name}":{{"type":"string","default":"{default}"}}}}}}"#
        );
        let user_options = jsonc::parse(&format!(r#"{{"{name}":"{user_value}"}}"#))
            .unwrap()
            .value();

        let m = FeatureManifest::parse(&random_name(), &user_options, &content).unwrap();

        assert_eq!(m.options, user_options);
    }

    #[test]
    fn when_parse_with_user_value_for_undeclared_option_then_options_have_both() {
        let (declared, undeclared, user_value) = (random_name(), random_name(), random_name());
        let content = format!(
            r#"{{"id":"f","options":{{"{declared}":{{"type":"boolean","default":true}}}}}}"#
        );
        let user_options = jsonc::parse(&format!(r#"{{"{undeclared}":"{user_value}"}}"#))
            .unwrap()
            .value();

        let m = FeatureManifest::parse(&random_name(), &user_options, &content).unwrap();

        assert_eq!(
            m.options,
            jsonc::parse(&format!(
                r#"{{"{declared}":true,"{undeclared}":"{user_value}"}}"#
            ))
            .unwrap()
            .value()
        );
    }

    #[test]
    fn when_parse_with_option_without_default_then_options_omit_the_option() {
        let name = random_name();
        let content = format!(r#"{{"id":"f","options":{{"{name}":{{"type":"string"}}}}}}"#);

        let m = FeatureManifest::parse(&random_name(), &Value::Object(vec![]), &content).unwrap();

        assert_eq!(m.options, Value::Object(vec![]));
    }

    #[test]
    fn when_parse_with_non_object_user_value_then_options_have_the_defaults() {
        let (name, default) = (random_name(), random_name());
        let content = format!(
            r#"{{"id":"f","options":{{"{name}":{{"type":"string","default":"{default}"}}}}}}"#
        );

        let m = FeatureManifest::parse(&random_name(), &Value::String(random_name()), &content)
            .unwrap();

        assert_eq!(
            m.options,
            jsonc::parse(&format!(r#"{{"{name}":"{default}"}}"#))
                .unwrap()
                .value()
        );
    }

    #[test]
    fn when_parse_without_declared_options_then_options_have_the_user_values() {
        let user_options = jsonc::parse(&format!(r#"{{"{}":"{}"}}"#, random_name(), random_name()))
            .unwrap()
            .value();

        let m = FeatureManifest::parse(&random_name(), &user_options, r#"{"id":"f"}"#).unwrap();

        assert_eq!(m.options, user_options);
    }

    #[test]
    fn when_parse_with_non_object_options_then_returns_error() {
        let content = format!(r#"{{"id":"f","options":"{}"}}"#, random_name());

        let result = FeatureManifest::parse(&random_name(), &Value::Object(vec![]), &content);

        assert!(result.is_err());
    }

    #[test]
    fn when_parse_with_non_object_option_then_returns_error() {
        let content = format!(
            r#"{{"id":"f","options":{{"{}":"{}"}}}}"#,
            random_name(),
            random_name()
        );

        let result = FeatureManifest::parse(&random_name(), &Value::Object(vec![]), &content);

        assert!(result.is_err());
    }

    #[test]
    fn when_parse_with_all_metadata_properties_then_metadata_has_all() {
        let (user_feature_id, text) = (random_name(), random_name());
        let properties = format!(
            r#""onCreateCommand": "{text}",
            "updateContentCommand": "{text}",
            "postCreateCommand": "{text}",
            "postStartCommand": "{text}",
            "postAttachCommand": "{text}",
            "init": true,
            "privileged": true,
            "capAdd": [],
            "securityOpt": [],
            "entrypoint": "{text}",
            "mounts": [],
            "customizations": {{}}"#
        );
        let m = FeatureManifest::parse(
            &user_feature_id,
            &Value::Object(vec![]),
            &format!(r#"{{"id": "f", {properties}}}"#),
        )
        .unwrap();
        let expected = jsonc::parse(&format!(r#"{{"id": "{user_feature_id}", {properties}}}"#))
            .unwrap()
            .value();
        assert_eq!(
            expected.as_object().unwrap().len(),
            METADATA_PROPERTIES.len() + 1
        );
        assert_eq!(m.metadata, expected.as_object().unwrap());
    }
}
