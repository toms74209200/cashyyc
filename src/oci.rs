use crate::devcontainer::jsonc;
use crate::err;
use crate::error::Result;

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
    let Ok(document) = jsonc::parse(json) else {
        return vec![];
    };
    let value = document.value();
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

fn parse_template_options(json: &str) -> Vec<TemplateOption> {
    let scalar = |value: &jsonc::Value| -> Option<String> {
        match value {
            jsonc::Value::String(s) => Some(s.clone()),
            jsonc::Value::Bool(b) => Some(b.to_string()),
            _ => None,
        }
    };
    let Ok(document) = jsonc::parse(json) else {
        return vec![];
    };
    let value = document.value();
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

const ALWAYS_OMITTED: [&str; 3] = ["devcontainer-template.json", "README.md", "NOTES.md"];
const DEVCONTAINER_JSON: &str = ".devcontainer/devcontainer.json";
const TEMPLATE_JSON: &str = "devcontainer-template.json";

#[derive(Clone, Debug, PartialEq)]
enum OptionalPath {
    File(String),
    Directory(String),
}

pub struct OptionValue {
    pub id: String,
    pub value: String,
}

pub struct TemplateFile {
    pub path: String,
    pub content: Vec<u8>,
}

pub struct ExtractedTemplate {
    entries: Vec<(String, Vec<u8>)>,
    devcontainer_json: String,
    optional_paths: Vec<OptionalPath>,
    pub optional_labels: Vec<String>,
    pub options: Vec<TemplateOption>,
}

pub struct SelectedTemplate<'a> {
    template: &'a ExtractedTemplate,
    pub paths: Vec<String>,
}

impl ExtractedTemplate {
    pub fn parse(entries: Vec<(String, Vec<u8>)>) -> Result<Self> {
        let content = |name: &str| {
            entries
                .iter()
                .find(|(path, _)| path == name)
                .map(|(_, content)| String::from_utf8_lossy(content).into_owned())
        };
        let devcontainer_json = content(DEVCONTAINER_JSON)
            .ok_or_else(|| err!("devcontainer.json not found in template"))?;
        let metadata = content(TEMPLATE_JSON).unwrap_or_default();
        let optional_paths: Vec<OptionalPath> = jsonc::parse(&metadata)
            .ok()
            .map(|document| document.value())
            .as_ref()
            .and_then(|manifest| manifest.get("optionalPaths"))
            .and_then(|paths| paths.as_array())
            .map(|paths| {
                paths
                    .iter()
                    .filter_map(|path| path.as_str())
                    .map(|raw| match raw.strip_suffix('*') {
                        Some(prefix) if prefix.ends_with('/') => {
                            OptionalPath::Directory(prefix.to_string())
                        }
                        _ => OptionalPath::File(raw.to_string()),
                    })
                    .collect()
            })
            .unwrap_or_default();
        Ok(Self {
            entries,
            devcontainer_json,
            optional_labels: optional_paths
                .iter()
                .map(|path| match path {
                    OptionalPath::File(file) => file.clone(),
                    OptionalPath::Directory(prefix) => format!("{prefix}*"),
                })
                .collect(),
            optional_paths,
            options: parse_template_options(&metadata),
        })
    }

    pub fn select(&self, included: &[usize]) -> SelectedTemplate<'_> {
        let omitted: Vec<&OptionalPath> = self
            .optional_paths
            .iter()
            .enumerate()
            .filter(|(i, _)| !included.contains(i))
            .map(|(_, path)| path)
            .collect();
        SelectedTemplate {
            template: self,
            paths: self
                .entries
                .iter()
                .map(|(path, _)| path)
                .filter(|path| !ALWAYS_OMITTED.contains(&path.as_str()))
                .filter(|path| {
                    !omitted.iter().any(|omit| match omit {
                        OptionalPath::File(file) => *path == file,
                        OptionalPath::Directory(prefix) => path.starts_with(prefix.as_str()),
                    })
                })
                .cloned()
                .collect(),
        }
    }
}

impl SelectedTemplate<'_> {
    pub fn render(
        &self,
        option_values: &[OptionValue],
        feature_ids: &[String],
    ) -> Result<Vec<TemplateFile>> {
        let devcontainer_json =
            build_devcontainer_json(&self.template.devcontainer_json, option_values, feature_ids)?;
        Ok(self
            .paths
            .iter()
            .map(|path| {
                let content = if path == DEVCONTAINER_JSON {
                    devcontainer_json.clone().into_bytes()
                } else {
                    let raw = self
                        .template
                        .entries
                        .iter()
                        .find(|(entry, _)| entry == path)
                        .map(|(_, content)| content.as_slice())
                        .unwrap_or_default();
                    match std::str::from_utf8(raw) {
                        Ok(text) => option_values
                            .iter()
                            .fold(text.to_string(), |acc, OptionValue { id, value }| {
                                acc.replace(&format!("${{templateOption:{id}}}"), value)
                            })
                            .into_bytes(),
                        Err(_) => raw.to_vec(),
                    }
                };
                TemplateFile {
                    path: path.clone(),
                    content,
                }
            })
            .collect())
    }
}

fn build_devcontainer_json(
    template_json: &str,
    option_values: &[OptionValue],
    feature_ids: &[String],
) -> Result<String> {
    let expanded = option_values.iter().fold(
        template_json.to_string(),
        |acc, OptionValue { id, value }| acc.replace(&format!("${{templateOption:{id}}}"), value),
    );
    let mut document = jsonc::parse(&expanded)?;
    let value = document.value();
    if value.as_object().is_none() {
        return Err(err!("template devcontainer.json is not a JSON object"));
    }
    if feature_ids.is_empty() {
        return Ok(expanded);
    }
    let without_tag = |reference: &str| {
        reference
            .rsplit_once(':')
            .map_or(reference, |(id, _)| id)
            .to_string()
    };
    let mut declared: Vec<String> = match value.get("features") {
        Some(features) => features
            .as_object()
            .ok_or_else(|| err!("template devcontainer.json 'features' is not a JSON object"))?
            .iter()
            .map(|(key, _)| key.clone())
            .collect(),
        None => vec![],
    };
    for id in feature_ids {
        let key = format!("ghcr.io/devcontainers/features/{id}:1");
        if declared
            .iter()
            .any(|existing| without_tag(existing) == without_tag(&key))
        {
            continue;
        }
        document
            .insert(&["features", &key], &jsonc::Value::Object(vec![]))
            .map_err(|e| err!("failed to declare feature {id}: {e}"))?;
        declared.push(key);
    }
    Ok(document.to_text())
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
        let value = jsonc::parse(&result).unwrap().value();
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
        let value = jsonc::parse(&result).unwrap().value();
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
        let value = jsonc::parse(&result).unwrap().value();
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
        let value = jsonc::parse(&result).unwrap().value();
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
        let value = jsonc::parse(&result).unwrap().value();
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
                OptionValue {
                    id: "imageVariant".to_string(),
                    value: "21-bookworm".to_string(),
                },
                OptionValue {
                    id: "installMaven".to_string(),
                    value: "true".to_string(),
                },
            ],
            &[],
        )
        .unwrap();
        let value = jsonc::parse(&result).unwrap().value();
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
        let result = build_devcontainer_json(
            template,
            &[OptionValue {
                id: "v".to_string(),
                value: "1".to_string(),
            }],
            &[],
        )
        .unwrap();
        let value = jsonc::parse(&result).unwrap().value();
        assert_eq!(value.get("image").and_then(|v| v.as_str()), Some("1-1"));
    }

    #[test]
    fn when_build_devcontainer_json_without_option_values_then_placeholder_remains() {
        let template = r#"{"image":"${templateOption:imageVariant}"}"#;
        let result = build_devcontainer_json(template, &[], &[]).unwrap();
        let value = jsonc::parse(&result).unwrap().value();
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
        let value = jsonc::parse(&result).unwrap().value();
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
        let value = jsonc::parse(&result).unwrap().value();
        assert_eq!(value.get("image").and_then(|v| v.as_str()), Some("alpine"));
    }

    #[test]
    fn when_build_devcontainer_json_with_url_in_string_then_preserves_it() {
        let template = r#"{"url":"https://example.com"}"#;
        let result = build_devcontainer_json(template, &[], &[]).unwrap();
        let value = jsonc::parse(&result).unwrap().value();
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

    #[test]
    fn when_parsing_a_template_without_devcontainer_json_then_it_is_rejected() {
        let entries = vec![("devcontainer-template.json".to_string(), b"{}".to_vec())];
        assert!(ExtractedTemplate::parse(entries).is_err());
    }

    #[test]
    fn when_parsing_a_template_then_its_manifest_is_read_from_its_own_entries() {
        let entries = vec![
            (
                "devcontainer-template.json".to_string(),
                br#"{"optionalPaths":[".github/*"]}"#.to_vec(),
            ),
            (DEVCONTAINER_JSON.to_string(), b"{}".to_vec()),
        ];
        let template = ExtractedTemplate::parse(entries).unwrap();
        assert_eq!(
            template.optional_paths,
            [OptionalPath::Directory(".github/".to_string())]
        );
    }

    #[test]
    fn when_parsing_a_manifest_then_each_optional_path_is_parsed_into_its_form() {
        let entries = vec![
            (
                TEMPLATE_JSON.to_string(),
                br#"{"optionalPaths":[".github/dependabot.yml",".github/*"]}"#.to_vec(),
            ),
            (DEVCONTAINER_JSON.to_string(), b"{}".to_vec()),
        ];
        assert_eq!(
            ExtractedTemplate::parse(entries).unwrap().optional_paths,
            [
                OptionalPath::File(".github/dependabot.yml".to_string()),
                OptionalPath::Directory(".github/".to_string()),
            ]
        );
    }

    #[test]
    fn when_a_manifest_declares_no_optional_paths_then_none_are_parsed() {
        for metadata in [r#"{"id":"java"}"#, r#"{"optionalPaths":"a"}"#, "invalid"] {
            let entries = vec![
                (TEMPLATE_JSON.to_string(), metadata.as_bytes().to_vec()),
                (DEVCONTAINER_JSON.to_string(), b"{}".to_vec()),
            ];
            assert!(
                ExtractedTemplate::parse(entries)
                    .unwrap()
                    .optional_paths
                    .is_empty(),
                "unexpected optional paths from {metadata}"
            );
        }
    }

    #[test]
    fn when_nothing_is_omitted_then_only_the_always_omitted_are_dropped() {
        let template = ExtractedTemplate::parse(
            [
                ".devcontainer/Dockerfile",
                DEVCONTAINER_JSON,
                ".github/dependabot.yml",
                "NOTES.md",
                "README.md",
                TEMPLATE_JSON,
            ]
            .iter()
            .map(|path| (path.to_string(), b"{}".to_vec()))
            .collect(),
        )
        .unwrap();
        assert_eq!(
            template
                .select(&(0..template.optional_labels.len()).collect::<Vec<_>>())
                .paths,
            [
                ".devcontainer/Dockerfile",
                DEVCONTAINER_JSON,
                ".github/dependabot.yml",
            ]
        );
    }

    #[test]
    fn when_a_file_is_omitted_then_that_path_is_dropped() {
        let template = ExtractedTemplate::parse(vec![
            (
                TEMPLATE_JSON.to_string(),
                br#"{"optionalPaths":[".github/dependabot.yml"]}"#.to_vec(),
            ),
            (DEVCONTAINER_JSON.to_string(), b"{}".to_vec()),
            (".github/dependabot.yml".to_string(), b"{}".to_vec()),
        ])
        .unwrap();
        assert_eq!(template.select(&[]).paths, [DEVCONTAINER_JSON]);
        assert_eq!(
            template.select(&[0]).paths,
            [DEVCONTAINER_JSON, ".github/dependabot.yml"]
        );
    }

    #[test]
    fn when_a_directory_is_omitted_then_everything_under_it_is_dropped() {
        let template = ExtractedTemplate::parse(vec![
            (
                TEMPLATE_JSON.to_string(),
                br#"{"optionalPaths":[".github/*"]}"#.to_vec(),
            ),
            (DEVCONTAINER_JSON.to_string(), b"{}".to_vec()),
            (".github/dependabot.yml".to_string(), b"{}".to_vec()),
            (".github/workflows/ci.yml".to_string(), b"{}".to_vec()),
        ])
        .unwrap();
        assert_eq!(template.select(&[]).paths, [DEVCONTAINER_JSON]);
    }

    #[test]
    fn when_a_file_is_omitted_then_a_path_that_merely_starts_with_it_is_kept() {
        let template = ExtractedTemplate::parse(vec![
            (
                TEMPLATE_JSON.to_string(),
                br#"{"optionalPaths":[".github/dependabot.yml"]}"#.to_vec(),
            ),
            (DEVCONTAINER_JSON.to_string(), b"{}".to_vec()),
            (".github/dependabot.yml.bak".to_string(), b"{}".to_vec()),
        ])
        .unwrap();
        assert_eq!(
            template.select(&[]).paths,
            [DEVCONTAINER_JSON, ".github/dependabot.yml.bak"]
        );
    }

    #[test]
    fn when_an_always_omitted_name_is_nested_then_it_is_kept() {
        let template = ExtractedTemplate::parse(
            [DEVCONTAINER_JSON, "docs/README.md"]
                .iter()
                .map(|path| (path.to_string(), b"{}".to_vec()))
                .collect(),
        )
        .unwrap();
        assert_eq!(
            template
                .select(&(0..template.optional_labels.len()).collect::<Vec<_>>())
                .paths,
            [DEVCONTAINER_JSON, "docs/README.md"]
        );
    }

    #[test]
    fn when_an_optional_path_is_shown_then_its_label_is_what_the_manifest_declared() {
        let entries = vec![
            (
                TEMPLATE_JSON.to_string(),
                br#"{"optionalPaths":[".github/dependabot.yml",".github/*"]}"#.to_vec(),
            ),
            (DEVCONTAINER_JSON.to_string(), b"{}".to_vec()),
        ];
        assert_eq!(
            ExtractedTemplate::parse(entries).unwrap().optional_labels,
            [".github/dependabot.yml", ".github/*"]
        );
    }

    #[test]
    fn when_rendering_then_every_placeholder_in_every_file_is_replaced() {
        let template = ExtractedTemplate::parse(vec![
            (
                DEVCONTAINER_JSON.to_string(),
                br#"{"image":"${templateOption:imageVariant}"}"#.to_vec(),
            ),
            (
                ".devcontainer/Dockerfile".to_string(),
                b"FROM cpp:3-${templateOption:imageVariant}\nARG V=\"${templateOption:cmake}\"\n"
                    .to_vec(),
            ),
        ])
        .unwrap();
        let files = template
            .select(&[])
            .render(
                &[
                    OptionValue {
                        id: "imageVariant".to_string(),
                        value: "debian-12".to_string(),
                    },
                    OptionValue {
                        id: "cmake".to_string(),
                        value: "none".to_string(),
                    },
                ],
                &[],
            )
            .unwrap();
        let dockerfile = files
            .iter()
            .find(|file| file.path == ".devcontainer/Dockerfile")
            .unwrap();
        assert_eq!(
            dockerfile.content,
            b"FROM cpp:3-debian-12\nARG V=\"none\"\n"
        );
        let config = files
            .iter()
            .find(|file| file.path == DEVCONTAINER_JSON)
            .unwrap();
        assert!(String::from_utf8_lossy(&config.content).contains("debian-12"));
    }

    #[test]
    fn when_rendering_an_option_that_has_no_value_then_its_placeholder_remains() {
        let template = ExtractedTemplate::parse(vec![
            (DEVCONTAINER_JSON.to_string(), b"{}".to_vec()),
            (
                ".devcontainer/Dockerfile".to_string(),
                b"FROM cpp:3-${templateOption:imageVariant}".to_vec(),
            ),
        ])
        .unwrap();
        let files = template.select(&[]).render(&[], &[]).unwrap();
        let dockerfile = files
            .iter()
            .find(|file| file.path == ".devcontainer/Dockerfile")
            .unwrap();
        assert_eq!(
            dockerfile.content,
            b"FROM cpp:3-${templateOption:imageVariant}"
        );
    }

    #[test]
    fn when_rendering_a_file_that_is_not_text_then_its_bytes_are_unchanged() {
        let bytes = vec![0x89, 0x50, 0x4e, 0x47, 0xff, 0xfe];
        let template = ExtractedTemplate::parse(vec![
            (DEVCONTAINER_JSON.to_string(), b"{}".to_vec()),
            (".devcontainer/logo.png".to_string(), bytes.clone()),
        ])
        .unwrap();
        let files = template
            .select(&[])
            .render(
                &[OptionValue {
                    id: "v".to_string(),
                    value: "1".to_string(),
                }],
                &[],
            )
            .unwrap();
        let logo = files
            .iter()
            .find(|file| file.path == ".devcontainer/logo.png")
            .unwrap();
        assert_eq!(logo.content, bytes);
    }

    #[test]
    fn when_rendering_with_features_then_devcontainer_json_declares_them() {
        let template = ExtractedTemplate::parse(vec![(
            DEVCONTAINER_JSON.to_string(),
            br#"{"image":"alpine"}"#.to_vec(),
        )])
        .unwrap();
        let files = template
            .select(&[])
            .render(&[], &["git".to_string()])
            .unwrap();
        let config = jsonc::parse(&String::from_utf8_lossy(&files[0].content))
            .unwrap()
            .value();
        assert!(
            config
                .get("features")
                .and_then(|features| features.get("ghcr.io/devcontainers/features/git:1"))
                .is_some()
        );
    }

    #[test]
    fn when_build_devcontainer_json_without_features_then_returns_the_expanded_template_text() {
        let template =
            "// header\n{\n\t// note\n\t\"image\": \"java:${templateOption:variant}\"\n}\n";
        let result = build_devcontainer_json(
            template,
            &[OptionValue {
                id: "variant".to_string(),
                value: "17".to_string(),
            }],
            &[],
        )
        .unwrap();
        assert_eq!(
            result,
            "// header\n{\n\t// note\n\t\"image\": \"java:17\"\n}\n"
        );
    }

    #[test]
    fn when_build_devcontainer_json_with_a_feature_then_keeps_the_comments_and_the_tab_indent() {
        let template = "// header\n{\n\t// note\n\t\"features\": {\n\t\t\"ghcr.io/devcontainers/features/java:1\": {}\n\t}\n}\n";
        let result = build_devcontainer_json(template, &[], &["git".to_string()]).unwrap();
        assert_eq!(
            result,
            "// header\n{\n\t// note\n\t\"features\": {\n\t\t\"ghcr.io/devcontainers/features/java:1\": {},\n\t\t\"ghcr.io/devcontainers/features/git:1\": {}\n\t}\n}\n"
        );
    }

    #[test]
    fn when_build_devcontainer_json_with_features_not_an_object_then_returns_error() {
        assert!(build_devcontainer_json(r#"{"features": 1}"#, &[], &["git".to_string()]).is_err());
    }
}
