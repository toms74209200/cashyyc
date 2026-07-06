use serde::Deserialize;

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

#[derive(Deserialize)]
struct TemplateEntry {
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: String,
}

#[derive(Deserialize)]
struct FeatureEntry {
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    description: String,
}

pub fn parse_templates(json: &str) -> Vec<Template> {
    let entries: Vec<serde_json::Value> = serde_json::from_str::<serde_json::Value>(json)
        .ok()
        .and_then(|v| v.get("templates")?.as_array().cloned())
        .unwrap_or_default();
    entries
        .into_iter()
        .filter_map(|v| serde_json::from_value::<TemplateEntry>(v).ok())
        .map(|e| Template {
            id: e.id,
            name: e.name,
            description: e.description,
        })
        .collect()
}

fn strip_json_comments(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut in_string = false;
    let mut escape = false;
    let bytes = input.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if in_string {
            out.push(bytes[i] as char);
            if escape {
                escape = false;
            } else if bytes[i] == b'\\' {
                escape = true;
            } else if bytes[i] == b'"' {
                in_string = false;
            }
            i += 1;
        } else if i + 1 < bytes.len() && bytes[i] == b'/' && bytes[i + 1] == b'/' {
            while i < bytes.len() && bytes[i] != b'\n' {
                i += 1;
            }
        } else {
            if bytes[i] == b'"' {
                in_string = true;
            }
            out.push(bytes[i] as char);
            i += 1;
        }
    }
    out
}

pub fn build_devcontainer_json(
    template_json: &str,
    feature_ids: &[String],
) -> anyhow::Result<String> {
    let stripped = strip_json_comments(template_json);
    let mut value: serde_json::Value = serde_json::from_str(&stripped)?;
    let obj = value
        .as_object_mut()
        .ok_or_else(|| anyhow::anyhow!("template devcontainer.json is not a JSON object"))?;
    if !feature_ids.is_empty() {
        let features: serde_json::Map<String, serde_json::Value> = feature_ids
            .iter()
            .map(|id| {
                (
                    format!("ghcr.io/devcontainers/features/{id}:1"),
                    serde_json::json!({}),
                )
            })
            .collect();
        obj.insert("features".to_string(), serde_json::Value::Object(features));
    }
    serde_json::to_string_pretty(&value).map_err(Into::into)
}

pub fn parse_features(json: &str) -> Vec<Feature> {
    let entries: Vec<serde_json::Value> = serde_json::from_str::<serde_json::Value>(json)
        .ok()
        .and_then(|v| v.get("features")?.as_array().cloned())
        .unwrap_or_default();
    entries
        .into_iter()
        .filter_map(|v| serde_json::from_value::<FeatureEntry>(v).ok())
        .map(|e| Feature {
            id: e.id,
            name: e.name,
            description: e.description,
        })
        .collect()
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
    fn when_build_devcontainer_json_with_no_features_then_omits_features_key() {
        let template = r#"{"image":"mcr.microsoft.com/devcontainers/rust:1-trixie"}"#;
        let result = build_devcontainer_json(template, &[]).unwrap();
        let value: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(value.get("features"), None);
        assert_eq!(
            value["image"],
            "mcr.microsoft.com/devcontainers/rust:1-trixie"
        );
    }

    #[test]
    fn when_build_devcontainer_json_with_one_feature_then_adds_features_key() {
        let template = r#"{"image":"mcr.microsoft.com/devcontainers/rust:1-trixie"}"#;
        let result = build_devcontainer_json(template, &["git".to_string()]).unwrap();
        let value: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(
            value["features"]["ghcr.io/devcontainers/features/git:1"],
            serde_json::json!({})
        );
    }

    #[test]
    fn when_build_devcontainer_json_with_multiple_features_then_adds_all() {
        let template = r#"{"image":"mcr.microsoft.com/devcontainers/rust:1-trixie"}"#;
        let result =
            build_devcontainer_json(template, &["git".to_string(), "github-cli".to_string()])
                .unwrap();
        let value: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(
            value["features"]["ghcr.io/devcontainers/features/git:1"],
            serde_json::json!({})
        );
        assert_eq!(
            value["features"]["ghcr.io/devcontainers/features/github-cli:1"],
            serde_json::json!({})
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
        let value: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(value["image"], "alpine");
    }

    #[test]
    fn when_strip_json_comments_preserves_url_in_string() {
        let input = r#"{"url":"https://example.com"}"#;
        assert_eq!(strip_json_comments(input), input);
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
