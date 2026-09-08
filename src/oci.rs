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

pub enum TemplateOption {
    Fixed { id: String, value: String },
    Choice { id: String, choices: Vec<String> },
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

pub fn parse_template_options(json: &str) -> Vec<TemplateOption> {
    let scalar = |value: &jsonc::Value| -> Option<String> {
        match value {
            jsonc::Value::String(s) => Some(s.clone()),
            jsonc::Value::Bool(b) => Some(b.to_string()),
            _ => None,
        }
    };
    let Ok(value) = jsonc::parse(json) else {
        return vec![];
    };
    let Some(options) = value.get("options").and_then(|v| v.as_object()) else {
        return vec![];
    };
    options
        .iter()
        .filter_map(|(id, spec)| {
            let default = spec.get("default").and_then(&scalar);
            let proposals: Vec<String> = match spec.get("type").and_then(|v| v.as_str()) {
                Some("boolean") => vec!["true".to_string(), "false".to_string()],
                _ => spec
                    .get("proposals")
                    .or_else(|| spec.get("enum"))
                    .and_then(|v| v.as_array())
                    .map(|items| items.iter().filter_map(&scalar).collect())
                    .unwrap_or_default(),
            };
            let mut choices: Vec<String> = default.iter().cloned().collect();
            choices.extend(
                proposals
                    .into_iter()
                    .filter(|c| Some(c) != default.as_ref()),
            );
            match choices.len() {
                0 => None,
                1 => Some(TemplateOption::Fixed {
                    id: id.clone(),
                    value: choices.remove(0),
                }),
                _ => Some(TemplateOption::Choice {
                    id: id.clone(),
                    choices,
                }),
            }
        })
        .collect()
}

pub fn build_devcontainer_json(
    template_json: &str,
    option_values: &[(String, String)],
    feature_ids: &[String],
) -> crate::error::Result<String> {
    let expanded = option_values
        .iter()
        .fold(template_json.to_string(), |acc, (id, value)| {
            acc.replace(&format!("${{templateOption:{id}}}"), value)
        });
    let value = jsonc::parse(&expanded)?;
    let mut members = match value {
        jsonc::Value::Object(members) => members,
        _ => {
            return Err(err!("template devcontainer.json is not a JSON object"));
        }
    };
    if !feature_ids.is_empty() {
        let without_tag = |reference: &str| {
            reference
                .rsplit_once(':')
                .map_or(reference, |(id, _)| id)
                .to_string()
        };
        let mut features = members
            .iter()
            .find(|(key, _)| key == "features")
            .and_then(|(_, value)| value.as_object())
            .unwrap_or_default()
            .to_vec();
        for id in feature_ids {
            let key = format!("ghcr.io/devcontainers/features/{id}:1");
            if !features
                .iter()
                .any(|(existing, _)| without_tag(existing) == without_tag(&key))
            {
                features.push((key, jsonc::Value::Object(vec![])));
            }
        }
        let features = jsonc::Value::Object(features);
        match members.iter_mut().find(|(key, _)| key == "features") {
            Some(entry) => entry.1 = features,
            None => members.push(("features".to_string(), features)),
        }
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
        let result = build_devcontainer_json(template, &[], &[]).unwrap();
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
        let result = build_devcontainer_json(template, &[], &["git".to_string()]).unwrap();
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
        let result = build_devcontainer_json(
            template,
            &[],
            &["git".to_string(), "github-cli".to_string()],
        )
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
    fn when_build_devcontainer_json_with_existing_features_then_keeps_them() {
        let template = r#"{"image":"alpine","features":{"old":{}}}"#;
        let result = build_devcontainer_json(template, &[], &["git".to_string()]).unwrap();
        let value = jsonc::parse(&result).unwrap();
        let features = value.get("features").unwrap();
        assert_eq!(
            features.get("old").cloned(),
            Some(jsonc::Value::Object(vec![]))
        );
        assert_eq!(
            features
                .get("ghcr.io/devcontainers/features/git:1")
                .cloned(),
            Some(jsonc::Value::Object(vec![]))
        );
    }

    #[test]
    fn when_build_devcontainer_json_with_feature_already_in_template_then_keeps_its_options() {
        let template = r#"{"image":"alpine","features":{"ghcr.io/devcontainers/features/java:1":{"installMaven":"true"}}}"#;
        let result = build_devcontainer_json(template, &[], &["java".to_string()]).unwrap();
        let value = jsonc::parse(&result).unwrap();
        let features = value.get("features").unwrap();
        assert_eq!(
            features
                .get("ghcr.io/devcontainers/features/java:1")
                .and_then(|f| f.get("installMaven"))
                .and_then(|v| v.as_str()),
            Some("true")
        );
        assert_eq!(
            features.as_object().map(<[(String, jsonc::Value)]>::len),
            Some(1)
        );
    }

    #[test]
    fn when_parse_template_options_with_proposals_then_default_comes_first() {
        let json = r#"{"options":{"imageVariant":{"type":"string","proposals":["3.20","3.21","3.22"],"default":"3.22"}}}"#;
        let options = parse_template_options(json);
        assert_eq!(options.len(), 1);
        let TemplateOption::Choice { id, choices } = &options[0] else {
            panic!("expected a choice");
        };
        assert_eq!(id, "imageVariant");
        assert_eq!(choices, &["3.22", "3.20", "3.21"]);
    }

    #[test]
    fn when_parse_template_options_with_a_single_choice_then_it_is_not_asked() {
        let json = r#"{"options":{"imageVariant":{"type":"string","default":"3.22"}}}"#;
        let options = parse_template_options(json);
        let TemplateOption::Fixed { id, value } = &options[0] else {
            panic!("expected a fixed value");
        };
        assert_eq!(id, "imageVariant");
        assert_eq!(value, "3.22");
    }

    #[test]
    fn when_parse_template_options_with_default_outside_proposals_then_default_is_added() {
        let json = r#"{"options":{"imageVariant":{"type":"string","proposals":["3.20"],"default":"3.22"}}}"#;
        let TemplateOption::Choice { choices, .. } = &parse_template_options(json)[0] else {
            panic!("expected a choice");
        };
        assert_eq!(choices, &["3.22", "3.20"]);
    }

    #[test]
    fn when_parse_template_options_with_enum_then_uses_enum_values() {
        let json = r#"{"options":{"variant":{"type":"string","enum":["a","b"],"default":"b"}}}"#;
        let TemplateOption::Choice { choices, .. } = &parse_template_options(json)[0] else {
            panic!("expected a choice");
        };
        assert_eq!(choices, &["b", "a"]);
    }

    #[test]
    fn when_parse_template_options_with_boolean_then_choices_are_true_and_false() {
        let json = r#"{"options":{"installMaven":{"type":"boolean","default":"false"}}}"#;
        let TemplateOption::Choice { choices, .. } = &parse_template_options(json)[0] else {
            panic!("expected a choice");
        };
        assert_eq!(choices, &["false", "true"]);
    }

    #[test]
    fn when_parse_template_options_with_boolean_literal_default_then_choices_are_strings() {
        let json = r#"{"options":{"installMaven":{"type":"boolean","default":true}}}"#;
        let TemplateOption::Choice { choices, .. } = &parse_template_options(json)[0] else {
            panic!("expected a choice");
        };
        assert_eq!(choices, &["true", "false"]);
    }

    #[test]
    fn when_parse_template_options_with_multiple_options_then_keeps_declaration_order() {
        let json = r#"{"options":{"imageVariant":{"type":"string","proposals":["3.20","3.21"],"default":"3.20"},"installMaven":{"type":"boolean","default":"false"}}}"#;
        let ids: Vec<String> = parse_template_options(json)
            .iter()
            .map(|o| match o {
                TemplateOption::Fixed { id, .. } | TemplateOption::Choice { id, .. } => id.clone(),
            })
            .collect();
        assert_eq!(ids, ["imageVariant", "installMaven"]);
    }

    #[test]
    fn when_parse_template_options_without_choices_then_skips_option() {
        let json = r#"{"options":{"name":{"type":"string","description":"free text"}}}"#;
        assert!(parse_template_options(json).is_empty());
    }

    #[test]
    fn when_parse_template_options_without_options_key_then_returns_empty() {
        assert!(parse_template_options(r#"{"id":"java"}"#).is_empty());
    }

    #[test]
    fn when_parse_template_options_with_invalid_json_then_returns_empty() {
        assert!(parse_template_options("invalid").is_empty());
    }

    #[test]
    fn when_build_devcontainer_json_with_option_values_then_replaces_every_placeholder() {
        let template = r#"{"image":"java:3-${templateOption:imageVariant}","features":{"java":{"installMaven":"${templateOption:installMaven}"}}}"#;
        let result = build_devcontainer_json(
            template,
            &[
                ("imageVariant".to_string(), "21-bookworm".to_string()),
                ("installMaven".to_string(), "true".to_string()),
            ],
            &[],
        )
        .unwrap();
        let value = jsonc::parse(&result).unwrap();
        assert_eq!(
            value.get("image").and_then(|v| v.as_str()),
            Some("java:3-21-bookworm")
        );
        assert_eq!(
            value
                .get("features")
                .and_then(|f| f.get("java"))
                .and_then(|j| j.get("installMaven"))
                .and_then(|v| v.as_str()),
            Some("true")
        );
    }

    #[test]
    fn when_build_devcontainer_json_with_repeated_placeholder_then_replaces_all_occurrences() {
        let template = r#"{"image":"${templateOption:v}-${templateOption:v}"}"#;
        let result =
            build_devcontainer_json(template, &[("v".to_string(), "1".to_string())], &[]).unwrap();
        let value = jsonc::parse(&result).unwrap();
        assert_eq!(value.get("image").and_then(|v| v.as_str()), Some("1-1"));
    }

    #[test]
    fn when_build_devcontainer_json_without_option_values_then_placeholder_remains() {
        let template = r#"{"image":"${templateOption:imageVariant}"}"#;
        let result = build_devcontainer_json(template, &[], &[]).unwrap();
        let value = jsonc::parse(&result).unwrap();
        assert_eq!(
            value.get("image").and_then(|v| v.as_str()),
            Some("${templateOption:imageVariant}")
        );
    }

    #[test]
    fn when_build_devcontainer_json_with_feature_at_another_version_then_keeps_the_template_one() {
        let template =
            r#"{"image":"alpine","features":{"ghcr.io/devcontainers/features/java:2":{}}}"#;
        let result = build_devcontainer_json(template, &[], &["java".to_string()]).unwrap();
        let value = jsonc::parse(&result).unwrap();
        let features = value.get("features").unwrap();
        assert_eq!(
            features.as_object().map(<[(String, jsonc::Value)]>::len),
            Some(1)
        );
        assert!(
            features
                .get("ghcr.io/devcontainers/features/java:2")
                .is_some()
        );
    }

    #[test]
    fn when_build_devcontainer_json_with_invalid_json_then_returns_err() {
        assert!(build_devcontainer_json("not json", &[], &[]).is_err());
    }

    #[test]
    fn when_build_devcontainer_json_with_non_object_then_returns_err() {
        assert!(build_devcontainer_json("[1,2,3]", &[], &["git".to_string()]).is_err());
    }

    #[test]
    fn when_build_devcontainer_json_with_comments_then_strips_them() {
        let template = "// comment\n{\"image\":\"alpine\"}\n";
        let result = build_devcontainer_json(template, &[], &[]).unwrap();
        let value = jsonc::parse(&result).unwrap();
        assert_eq!(value.get("image").and_then(|v| v.as_str()), Some("alpine"));
    }

    #[test]
    fn when_build_devcontainer_json_with_url_in_string_then_preserves_it() {
        let template = r#"{"url":"https://example.com"}"#;
        let result = build_devcontainer_json(template, &[], &[]).unwrap();
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
