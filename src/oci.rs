use crate::devcontainer::jsonc;
use crate::err;

pub struct Template {
    pub id: String,
    pub name: String,
    pub description: String,
}

pub struct Feature {
    pub id: String,
    pub name: String,
    pub description: String,
}

pub fn parse_templates(json: &str) -> Vec<Template> {
    parse_collection(json, "templates")
        .into_iter()
        .map(|(id, name, description)| Template {
            id,
            name,
            description,
        })
        .collect()
}

pub fn parse_features(json: &str) -> Vec<Feature> {
    parse_collection(json, "features")
        .into_iter()
        .map(|(id, name, description)| Feature {
            id,
            name,
            description,
        })
        .collect()
}

fn parse_collection(json: &str, key: &str) -> Vec<(String, String, String)> {
    let Ok(value) = jsonc::parse(json) else {
        return vec![];
    };
    let Some(entries) = value.get(key).and_then(|v| v.as_array()) else {
        return vec![];
    };
    entries
        .iter()
        .filter_map(|entry| {
            let id = entry.get("id")?.as_str()?.to_string();
            let field = |name: &str| {
                entry
                    .get(name)
                    .and_then(|v| v.as_str())
                    .unwrap_or_default()
                    .to_string()
            };
            Some((id, field("name"), field("description")))
        })
        .collect()
}

pub fn build_devcontainer_json(
    template_json: &str,
    feature_ids: &[String],
) -> crate::error::Result<String> {
    let value = jsonc::parse(template_json)?;
    let mut members = match value {
        jsonc::Value::Object(members) => members,
        _ => {
            return Err(err!("template devcontainer.json is not a JSON object"));
        }
    };
    if !feature_ids.is_empty() {
        members.retain(|(key, _)| key != "features");
        let features = feature_ids
            .iter()
            .map(|id| {
                (
                    format!("ghcr.io/devcontainers/features/{id}:1"),
                    jsonc::Value::Object(vec![]),
                )
            })
            .collect();
        members.push(("features".to_string(), jsonc::Value::Object(features)));
    }
    Ok(jsonc::Value::Object(members).to_json_pretty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn when_parse_templates_with_collection_then_returns_all_templates() {
        let json = r#"{"templates":[{"id":"go","name":"Go","description":"Go template"},{"id":"rust","name":"Rust","description":"Rust template"}]}"#;
        let templates = parse_templates(json);
        assert_eq!(templates.len(), 2);
        assert_eq!(templates[0].id, "go");
        assert_eq!(templates[0].name, "Go");
        assert_eq!(templates[1].id, "rust");
    }

    #[test]
    fn when_parse_templates_with_missing_name_then_defaults_to_empty() {
        let json = r#"{"templates":[{"id":"go"}]}"#;
        let templates = parse_templates(json);
        assert_eq!(templates[0].name, "");
    }

    #[test]
    fn when_parse_templates_with_invalid_json_then_returns_empty() {
        assert!(parse_templates("invalid").is_empty());
    }

    #[test]
    fn when_parse_templates_with_missing_key_then_returns_empty() {
        let json = r#"{"other":[]}"#;
        assert!(parse_templates(json).is_empty());
    }

    #[test]
    fn when_parse_templates_with_non_array_then_returns_empty() {
        let json = r#"{"templates":"not_array"}"#;
        assert!(parse_templates(json).is_empty());
    }

    #[test]
    fn when_parse_templates_with_invalid_entry_then_skips_it() {
        let json =
            r#"{"templates":[{"id":"go","name":"Go","description":"Go template"},{"name":"bad"}]}"#;
        let templates = parse_templates(json);
        assert_eq!(templates.len(), 1);
        assert_eq!(templates[0].id, "go");
    }

    #[test]
    fn when_parse_templates_with_non_string_id_then_skips_it() {
        let json = r#"{"templates":[{"id":1},{"id":"go"}]}"#;
        let templates = parse_templates(json);
        assert_eq!(templates.len(), 1);
        assert_eq!(templates[0].id, "go");
    }

    #[test]
    fn when_build_devcontainer_json_with_no_features_then_omits_features_key() {
        let template = r#"{"image":"mcr.microsoft.com/devcontainers/rust:1-trixie"}"#;
        let result = build_devcontainer_json(template, &[]).unwrap();
        let value = jsonc::parse(&result).unwrap();
        assert_eq!(value.get("features"), None);
        assert_eq!(
            value.get("image").and_then(|v| v.as_str()),
            Some("mcr.microsoft.com/devcontainers/rust:1-trixie")
        );
    }

    #[test]
    fn when_build_devcontainer_json_with_one_feature_then_adds_features_key() {
        let template = r#"{"image":"mcr.microsoft.com/devcontainers/rust:1-trixie"}"#;
        let result = build_devcontainer_json(template, &["git".to_string()]).unwrap();
        let value = jsonc::parse(&result).unwrap();
        assert_eq!(
            value
                .get("features")
                .and_then(|f| f.get("ghcr.io/devcontainers/features/git:1"))
                .cloned(),
            Some(jsonc::Value::Object(vec![]))
        );
    }

    #[test]
    fn when_build_devcontainer_json_with_multiple_features_then_adds_all() {
        let template = r#"{"image":"mcr.microsoft.com/devcontainers/rust:1-trixie"}"#;
        let result =
            build_devcontainer_json(template, &["git".to_string(), "github-cli".to_string()])
                .unwrap();
        let value = jsonc::parse(&result).unwrap();
        let features = value.get("features").unwrap();
        assert_eq!(
            features
                .get("ghcr.io/devcontainers/features/git:1")
                .cloned(),
            Some(jsonc::Value::Object(vec![]))
        );
        assert_eq!(
            features
                .get("ghcr.io/devcontainers/features/github-cli:1")
                .cloned(),
            Some(jsonc::Value::Object(vec![]))
        );
    }

    #[test]
    fn when_build_devcontainer_json_with_existing_features_then_replaces_key() {
        let template = r#"{"image":"alpine","features":{"old":{}}}"#;
        let result = build_devcontainer_json(template, &["git".to_string()]).unwrap();
        let value = jsonc::parse(&result).unwrap();
        let features = value.get("features").unwrap();
        assert_eq!(features.get("old"), None);
        assert_eq!(
            features
                .get("ghcr.io/devcontainers/features/git:1")
                .cloned(),
            Some(jsonc::Value::Object(vec![]))
        );
    }

    #[test]
    fn when_build_devcontainer_json_with_invalid_json_then_returns_err() {
        assert!(build_devcontainer_json("not json", &[]).is_err());
    }

    #[test]
    fn when_build_devcontainer_json_with_non_object_then_returns_err() {
        assert!(build_devcontainer_json("[1,2,3]", &["git".to_string()]).is_err());
    }

    #[test]
    fn when_build_devcontainer_json_with_comments_then_strips_them() {
        let template = "// comment\n{\"image\":\"alpine\"}\n";
        let result = build_devcontainer_json(template, &[]).unwrap();
        let value = jsonc::parse(&result).unwrap();
        assert_eq!(value.get("image").and_then(|v| v.as_str()), Some("alpine"));
    }

    #[test]
    fn when_build_devcontainer_json_with_url_in_string_then_preserves_it() {
        let template = r#"{"url":"https://example.com"}"#;
        let result = build_devcontainer_json(template, &[]).unwrap();
        let value = jsonc::parse(&result).unwrap();
        assert_eq!(
            value.get("url").and_then(|v| v.as_str()),
            Some("https://example.com")
        );
    }

    #[test]
    fn when_parse_features_with_collection_then_returns_all_features() {
        let json = r#"{"features":[{"id":"git","name":"Git","description":"Git feature"},{"id":"node","name":"Node.js","description":"Node feature"}]}"#;
        let features = parse_features(json);
        assert_eq!(features.len(), 2);
        assert_eq!(features[0].id, "git");
        assert_eq!(features[1].id, "node");
    }

    #[test]
    fn when_parse_features_with_missing_name_then_defaults_to_empty() {
        let json = r#"{"features":[{"id":"git"}]}"#;
        let features = parse_features(json);
        assert_eq!(features[0].name, "");
    }

    #[test]
    fn when_parse_features_with_invalid_json_then_returns_empty() {
        assert!(parse_features("invalid").is_empty());
    }

    #[test]
    fn when_parse_features_with_missing_key_then_returns_empty() {
        let json = r#"{"other":[]}"#;
        assert!(parse_features(json).is_empty());
    }

    #[test]
    fn when_parse_features_with_non_array_then_returns_empty() {
        let json = r#"{"features":"not_array"}"#;
        assert!(parse_features(json).is_empty());
    }

    #[test]
    fn when_parse_features_with_invalid_entry_then_skips_it() {
        let json = r#"{"features":[{"id":"git","name":"Git","description":"Git feature"},{"name":"bad"}]}"#;
        let features = parse_features(json);
        assert_eq!(features.len(), 1);
        assert_eq!(features[0].id, "git");
    }
}
