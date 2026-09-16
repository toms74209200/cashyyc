use super::jsonc::Value;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Metadata(Vec<Value>);

impl From<Value> for Metadata {
    fn from(value: Value) -> Self {
        let entries = match value {
            Value::Array(items) => items,
            entry @ Value::Object(_) => vec![entry],
            _ => vec![],
        };
        Self(
            entries
                .into_iter()
                .filter(|e| !matches!(e, Value::Object(members) if members.is_empty()))
                .collect(),
        )
    }
}

impl Metadata {
    pub const LABEL: &str = "devcontainer.metadata";

    pub fn for_container<'a>(
        image: Metadata,
        features: impl IntoIterator<Item = &'a Metadata>,
        config: &Metadata,
    ) -> Self {
        let mut entries = image.0;
        entries.extend(features.into_iter().flat_map(|f| f.0.iter().cloned()));
        entries.extend(config.0.iter().cloned());
        Self(entries)
    }

    pub fn to_docker_args(&self) -> Vec<String> {
        vec![
            "--label".to_string(),
            format!("{}={}", Self::LABEL, Value::Array(self.0.clone())),
        ]
    }

    pub fn to_compose_override(&self, content: &str, service: &str) -> String {
        let label = format!("{}={}", Self::LABEL, Value::Array(self.0.clone()))
            .replace('$', "$$")
            .replace('\'', "''");
        content.replacen(
            &format!("  '{service}':\n"),
            &format!("  '{service}':\n    labels:\n      - '{label}'\n"),
            1,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devcontainer::jsonc;
    use random_string::{CharacterType, generate_random_string};
    use std::fs::File;

    fn random_word() -> String {
        generate_random_string(
            8,
            &[CharacterType::Lowercase, CharacterType::Numeric],
            "",
            &mut File::open("/dev/urandom").unwrap(),
        )
    }

    #[test]
    fn when_from_array_then_returns_its_entries_in_order() {
        let (feature, user) = (random_word(), random_word());
        let value = jsonc::parse(&format!(
            r#"[{{"id":"{feature}"}},{{"remoteUser":"{user}"}}]"#
        ))
        .unwrap()
        .value();
        assert_eq!(
            Metadata::from(value),
            Metadata(vec![
                jsonc::parse(&format!(r#"{{"id":"{feature}"}}"#))
                    .unwrap()
                    .value(),
                jsonc::parse(&format!(r#"{{"remoteUser":"{user}"}}"#))
                    .unwrap()
                    .value(),
            ])
        );
    }

    #[test]
    fn when_from_object_then_returns_it_as_one_entry() {
        let user = random_word();
        let value = jsonc::parse(&format!(r#"{{"remoteUser":"{user}"}}"#))
            .unwrap()
            .value();
        assert_eq!(Metadata::from(value.clone()), Metadata(vec![value]));
    }

    #[test]
    fn when_from_empty_object_then_returns_empty() {
        let value = jsonc::parse("{}").unwrap().value();
        assert_eq!(Metadata::from(value), Metadata::default());
    }

    #[test]
    fn when_from_array_with_empty_objects_then_returns_without_them() {
        let user = random_word();
        let value = jsonc::parse(&format!(r#"[{{}},{{"remoteUser":"{user}"}},{{}}]"#))
            .unwrap()
            .value();
        assert_eq!(
            Metadata::from(value),
            Metadata(vec![
                jsonc::parse(&format!(r#"{{"remoteUser":"{user}"}}"#))
                    .unwrap()
                    .value()
            ])
        );
    }

    #[test]
    fn when_from_non_entry_value_then_returns_empty() {
        assert_eq!(
            Metadata::from(Value::String(random_word())),
            Metadata::default()
        );
        assert_eq!(Metadata::from(Value::Null), Metadata::default());
    }

    #[test]
    fn when_for_container_then_returns_image_features_config_in_order() {
        let (base, feature_a, feature_b, user) =
            (random_word(), random_word(), random_word(), random_word());
        let image = Metadata::from(
            jsonc::parse(&format!(r#"[{{"remoteUser":"{base}"}}]"#))
                .unwrap()
                .value(),
        );
        let features = [
            Metadata::from(
                jsonc::parse(&format!(r#"{{"id":"{feature_a}"}}"#))
                    .unwrap()
                    .value(),
            ),
            Metadata::from(
                jsonc::parse(&format!(r#"{{"id":"{feature_b}"}}"#))
                    .unwrap()
                    .value(),
            ),
        ];
        let config = Metadata::from(
            jsonc::parse(&format!(r#"{{"remoteUser":"{user}"}}"#))
                .unwrap()
                .value(),
        );
        assert_eq!(
            Metadata::for_container(image, &features, &config),
            Metadata::from(
                jsonc::parse(&format!(
                    r#"[{{"remoteUser":"{base}"}},{{"id":"{feature_a}"}},{{"id":"{feature_b}"}},{{"remoteUser":"{user}"}}]"#
                ))
                .unwrap()
                .value()
            )
        );
    }

    #[test]
    fn when_for_container_with_empty_parts_then_returns_empty() {
        assert_eq!(
            Metadata::for_container(Metadata::default(), [], &Metadata::default()),
            Metadata::default()
        );
    }

    #[test]
    fn when_to_docker_args_then_returns_label_with_compact_json() {
        let user = random_word();
        let metadata = Metadata::from(
            jsonc::parse(&format!(r#"[{{ "remoteUser" : "{user}" }}]"#))
                .unwrap()
                .value(),
        );
        assert_eq!(
            metadata.to_docker_args(),
            vec![
                "--label".to_string(),
                format!(r#"devcontainer.metadata=[{{"remoteUser":"{user}"}}]"#),
            ]
        );
    }

    #[test]
    fn when_to_docker_args_with_empty_metadata_then_returns_label_with_empty_array() {
        assert_eq!(
            Metadata::default().to_docker_args(),
            vec![
                "--label".to_string(),
                "devcontainer.metadata=[]".to_string()
            ]
        );
    }

    #[test]
    fn when_to_compose_override_then_returns_label_under_service() {
        let (service, user) = (random_word(), random_word());
        let metadata = Metadata::from(
            jsonc::parse(&format!(r#"{{"remoteUser":"{user}"}}"#))
                .unwrap()
                .value(),
        );
        let content = format!("services:\n  '{service}':\n    entrypoint: [\"/bin/sh\"]\n");
        assert_eq!(
            metadata.to_compose_override(&content, &service),
            format!(
                "services:\n  '{service}':\n    labels:\n      - 'devcontainer.metadata=[{{\"remoteUser\":\"{user}\"}}]'\n    entrypoint: [\"/bin/sh\"]\n"
            )
        );
    }

    #[test]
    fn when_to_compose_override_with_dollar_then_returns_it_escaped_for_compose() {
        let (service, command) = (random_word(), random_word());
        let metadata = Metadata::from(
            jsonc::parse(&format!(
                r#"{{"postCreateCommand":"{command} ${{localWorkspaceFolder}}"}}"#
            ))
            .unwrap()
            .value(),
        );
        let content = format!("services:\n  '{service}':\n");
        assert!(
            metadata
                .to_compose_override(&content, &service)
                .contains(&format!("{command} $${{localWorkspaceFolder}}"))
        );
    }

    #[test]
    fn when_to_compose_override_with_single_quote_then_returns_it_escaped_for_yaml() {
        let (service, command) = (random_word(), random_word());
        let metadata = Metadata::from(
            jsonc::parse(&format!(r#"{{"postCreateCommand":"echo '{command}'"}}"#))
                .unwrap()
                .value(),
        );
        let content = format!("services:\n  '{service}':\n");
        assert!(
            metadata
                .to_compose_override(&content, &service)
                .contains(&format!("echo ''{command}''"))
        );
    }

    #[test]
    fn when_to_compose_override_for_other_service_then_returns_content_unchanged() {
        let (service, other) = (random_word(), random_word());
        let content = format!("services:\n  '{service}':\n");
        assert_eq!(
            Metadata::default().to_compose_override(&content, &other),
            content
        );
    }
}
