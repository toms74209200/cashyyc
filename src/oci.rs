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
        let json = r#"{"templates":[{"id":"go","name":"Go","description":"Go template"},{"name":"bad"}]}"#;
        let templates = parse_templates(json);
        assert_eq!(templates.len(), 1);
        assert_eq!(templates[0].id, "go");
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
