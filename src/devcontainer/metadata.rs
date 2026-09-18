use super::config::CommonConfig;
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

    pub fn lifecycle_commands(&self, key: &str) -> Vec<Value> {
        self.0
            .iter()
            .filter_map(|entry| entry.get(key))
            .filter(|value| !matches!(value, Value::Null))
            .cloned()
            .collect()
    }

    pub fn merge_into(&self, config: &CommonConfig) -> CommonConfig {
        self.0
            .iter()
            .filter_map(CommonConfig::from_value)
            .chain(std::iter::once(config.clone()))
            .reduce(|earlier, later| CommonConfig {
                init: match (earlier.init, later.init) {
                    (Some(true), _) | (_, Some(true)) => Some(true),
                    (None, None) => None,
                    _ => Some(false),
                },
                privileged: match (earlier.privileged, later.privileged) {
                    (Some(true), _) | (_, Some(true)) => Some(true),
                    (None, None) => None,
                    _ => Some(false),
                },
                cap_add: earlier.cap_add.iter().chain(&later.cap_add).fold(
                    vec![],
                    |mut merged, cap| {
                        if !merged.contains(cap) {
                            merged.push(cap.clone());
                        }
                        merged
                    },
                ),
                security_opt: earlier.security_opt.iter().chain(&later.security_opt).fold(
                    vec![],
                    |mut merged, opt| {
                        if !merged.contains(opt) {
                            merged.push(opt.clone());
                        }
                        merged
                    },
                ),
                mounts: {
                    let overridden: Vec<&str> = later
                        .mounts
                        .iter()
                        .filter_map(|m| mount_target(m))
                        .collect();
                    earlier
                        .mounts
                        .iter()
                        .filter(|m| !mount_target(m).is_some_and(|t| overridden.contains(&t)))
                        .chain(&later.mounts)
                        .cloned()
                        .collect()
                },
                container_env: earlier
                    .container_env
                    .iter()
                    .chain(&later.container_env)
                    .map(|(key, value)| (key.clone(), value.clone()))
                    .collect(),
                remote_env: match (&earlier.remote_env, &later.remote_env) {
                    (None, None) => None,
                    (earlier_env, later_env) => Some(
                        earlier_env
                            .iter()
                            .chain(later_env)
                            .flatten()
                            .map(|(key, value)| (key.clone(), value.clone()))
                            .collect(),
                    ),
                },
                container_user: later
                    .container_user
                    .clone()
                    .or_else(|| earlier.container_user.clone()),
                remote_user: later
                    .remote_user
                    .clone()
                    .or_else(|| earlier.remote_user.clone()),
                update_remote_user_uid: later
                    .update_remote_user_uid
                    .or(earlier.update_remote_user_uid),
                user_env_probe: later
                    .user_env_probe
                    .clone()
                    .or_else(|| earlier.user_env_probe.clone()),
                override_command: later.override_command.or(earlier.override_command),
                wait_for: later.wait_for.clone().or_else(|| earlier.wait_for.clone()),
                ..later
            })
            .unwrap_or_else(|| config.clone())
    }
}

pub fn mount_target(mount: &str) -> Option<&str> {
    mount.split(',').find_map(|field| {
        let (key, value) = field.split_once('=')?;
        matches!(key.trim(), "target" | "dst" | "destination").then_some(value)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devcontainer::jsonc;
    use crate::devcontainer::{UserEnvProbe, WaitFor};
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

    #[test]
    fn when_merge_into_with_no_entries_then_returns_the_config_as_is() {
        let config = CommonConfig::from_value(
            &jsonc::parse(&format!(r#"{{"remoteUser":"{}"}}"#, random_word()))
                .unwrap()
                .value(),
        )
        .unwrap();
        assert_eq!(Metadata::default().merge_into(&config), config);
    }

    #[test]
    fn when_merge_into_with_remote_user_only_in_image_then_returns_the_image_user() {
        let user = random_word();
        let merged = Metadata::from(
            jsonc::parse(&format!(r#"[{{"remoteUser":"{user}"}}]"#))
                .unwrap()
                .value(),
        )
        .merge_into(&CommonConfig::from_value(&jsonc::parse("{}").unwrap().value()).unwrap());
        assert_eq!(merged.remote_user, Some(user));
    }

    #[test]
    fn when_merge_into_with_remote_user_in_both_then_returns_the_config_user() {
        let (image_user, config_user) = (random_word(), random_word());
        let merged = Metadata::from(
            jsonc::parse(&format!(r#"[{{"remoteUser":"{image_user}"}}]"#))
                .unwrap()
                .value(),
        )
        .merge_into(
            &CommonConfig::from_value(
                &jsonc::parse(&format!(r#"{{"remoteUser":"{config_user}"}}"#))
                    .unwrap()
                    .value(),
            )
            .unwrap(),
        );
        assert_eq!(merged.remote_user, Some(config_user));
    }

    #[test]
    fn when_merge_into_with_multiple_entries_then_returns_the_last_value() {
        let (first, last) = (random_word(), random_word());
        let merged = Metadata::from(
            jsonc::parse(&format!(
                r#"[{{"remoteUser":"{first}"}},{{"remoteUser":"{last}"}}]"#
            ))
            .unwrap()
            .value(),
        )
        .merge_into(&CommonConfig::from_value(&jsonc::parse("{}").unwrap().value()).unwrap());
        assert_eq!(merged.remote_user, Some(last));
    }

    #[test]
    fn when_merge_into_with_init_true_in_image_then_returns_init_true() {
        let merged = Metadata::from(jsonc::parse(r#"[{"init":true}]"#).unwrap().value())
            .merge_into(&CommonConfig::from_value(&jsonc::parse("{}").unwrap().value()).unwrap());
        assert_eq!(merged.init, Some(true));
    }

    #[test]
    fn when_merge_into_with_privileged_true_in_image_and_false_in_config_then_returns_privileged_true()
     {
        let merged = Metadata::from(jsonc::parse(r#"[{"privileged":true}]"#).unwrap().value())
            .merge_into(
                &CommonConfig::from_value(
                    &jsonc::parse(r#"{"privileged":false}"#).unwrap().value(),
                )
                .unwrap(),
            );
        assert_eq!(merged.privileged, Some(true));
    }

    #[test]
    fn when_merge_into_with_init_false_in_both_then_returns_init_false() {
        let merged = Metadata::from(jsonc::parse(r#"[{"init":false}]"#).unwrap().value())
            .merge_into(
                &CommonConfig::from_value(&jsonc::parse(r#"{"init":false}"#).unwrap().value())
                    .unwrap(),
            );
        assert_eq!(merged.init, Some(false));
    }

    #[test]
    fn when_merge_into_with_cap_add_in_both_then_returns_the_union_without_duplicates() {
        let (shared, image_only, config_only) = (random_word(), random_word(), random_word());
        let merged = Metadata::from(
            jsonc::parse(&format!(r#"[{{"capAdd":["{shared}","{image_only}"]}}]"#))
                .unwrap()
                .value(),
        )
        .merge_into(
            &CommonConfig::from_value(
                &jsonc::parse(&format!(r#"{{"capAdd":["{shared}","{config_only}"]}}"#))
                    .unwrap()
                    .value(),
            )
            .unwrap(),
        );
        assert_eq!(merged.cap_add, vec![shared, image_only, config_only]);
    }

    #[test]
    fn when_merge_into_with_security_opt_in_image_only_then_returns_it() {
        let opt = random_word();
        let merged = Metadata::from(
            jsonc::parse(&format!(r#"[{{"securityOpt":["{opt}"]}}]"#))
                .unwrap()
                .value(),
        )
        .merge_into(&CommonConfig::from_value(&jsonc::parse("{}").unwrap().value()).unwrap());
        assert_eq!(merged.security_opt, vec![opt]);
    }

    #[test]
    fn when_merge_into_with_container_env_of_the_same_key_then_returns_the_config_value() {
        let (key, image_value, config_value) = (random_word(), random_word(), random_word());
        let merged = Metadata::from(
            jsonc::parse(&format!(
                r#"[{{"containerEnv":{{"{key}":"{image_value}"}}}}]"#
            ))
            .unwrap()
            .value(),
        )
        .merge_into(
            &CommonConfig::from_value(
                &jsonc::parse(&format!(
                    r#"{{"containerEnv":{{"{key}":"{config_value}"}}}}"#
                ))
                .unwrap()
                .value(),
            )
            .unwrap(),
        );
        assert_eq!(merged.container_env.get(&key), Some(&config_value));
    }

    #[test]
    fn when_merge_into_with_container_env_of_different_keys_then_returns_both() {
        let (image_key, config_key, value) = (random_word(), random_word(), random_word());
        let merged = Metadata::from(
            jsonc::parse(&format!(
                r#"[{{"containerEnv":{{"{image_key}":"{value}"}}}}]"#
            ))
            .unwrap()
            .value(),
        )
        .merge_into(
            &CommonConfig::from_value(
                &jsonc::parse(&format!(
                    r#"{{"containerEnv":{{"{config_key}":"{value}"}}}}"#
                ))
                .unwrap()
                .value(),
            )
            .unwrap(),
        );
        assert_eq!(merged.container_env.get(&image_key), Some(&value));
        assert_eq!(merged.container_env.get(&config_key), Some(&value));
    }

    #[test]
    fn when_merge_into_with_mounts_of_the_same_target_then_returns_the_config_mount_only() {
        let (target, image_source, config_source) = (random_word(), random_word(), random_word());
        let merged = Metadata::from(
            jsonc::parse(&format!(
                r#"[{{"mounts":["type=bind,src=/{image_source},dst=/{target}"]}}]"#
            ))
            .unwrap()
            .value(),
        )
        .merge_into(
            &CommonConfig::from_value(
                &jsonc::parse(&format!(
                    r#"{{"mounts":["type=bind,src=/{config_source},dst=/{target}"]}}"#
                ))
                .unwrap()
                .value(),
            )
            .unwrap(),
        );
        assert_eq!(
            merged.mounts,
            vec![format!("type=bind,src=/{config_source},dst=/{target}")]
        );
    }

    #[test]
    fn when_merge_into_with_mounts_of_different_targets_then_returns_the_image_mount_first() {
        let (image_target, config_target, source) = (random_word(), random_word(), random_word());
        let merged = Metadata::from(
            jsonc::parse(&format!(
                r#"[{{"mounts":["type=bind,src=/{source},dst=/{image_target}"]}}]"#
            ))
            .unwrap()
            .value(),
        )
        .merge_into(
            &CommonConfig::from_value(
                &jsonc::parse(&format!(
                    r#"{{"mounts":["type=bind,src=/{source},dst=/{config_target}"]}}"#
                ))
                .unwrap()
                .value(),
            )
            .unwrap(),
        );
        assert_eq!(
            merged.mounts,
            vec![
                format!("type=bind,src=/{source},dst=/{image_target}"),
                format!("type=bind,src=/{source},dst=/{config_target}"),
            ]
        );
    }

    #[test]
    fn when_merge_into_with_remote_env_in_both_then_returns_them_merged() {
        let (image_key, config_key, value) = (random_word(), random_word(), random_word());
        let merged = Metadata::from(
            jsonc::parse(&format!(r#"[{{"remoteEnv":{{"{image_key}":"{value}"}}}}]"#))
                .unwrap()
                .value(),
        )
        .merge_into(
            &CommonConfig::from_value(
                &jsonc::parse(&format!(r#"{{"remoteEnv":{{"{config_key}":"{value}"}}}}"#))
                    .unwrap()
                    .value(),
            )
            .unwrap(),
        );
        let remote_env = merged.remote_env.unwrap();
        assert_eq!(remote_env.get(&image_key), Some(&Some(value.clone())));
        assert_eq!(remote_env.get(&config_key), Some(&Some(value)));
    }

    #[test]
    fn when_merge_into_with_wait_for_only_in_image_then_returns_the_image_wait_for() {
        let merged = Metadata::from(
            jsonc::parse(r#"[{"waitFor":"onCreateCommand"}]"#)
                .unwrap()
                .value(),
        )
        .merge_into(&CommonConfig::from_value(&jsonc::parse("{}").unwrap().value()).unwrap());
        assert_eq!(merged.wait_for, Some(WaitFor::OnCreateCommand));
    }

    #[test]
    fn when_merge_into_with_user_env_probe_only_in_image_then_returns_the_image_probe() {
        let merged = Metadata::from(
            jsonc::parse(r#"[{"userEnvProbe":"loginShell"}]"#)
                .unwrap()
                .value(),
        )
        .merge_into(&CommonConfig::from_value(&jsonc::parse("{}").unwrap().value()).unwrap());
        assert_eq!(merged.user_env_probe, Some(UserEnvProbe::LoginShell));
    }

    #[test]
    fn when_merge_into_with_post_create_command_in_image_then_returns_the_config_command() {
        let (image_command, config_command) = (random_word(), random_word());
        let merged = Metadata::from(
            jsonc::parse(&format!(r#"[{{"postCreateCommand":"{image_command}"}}]"#))
                .unwrap()
                .value(),
        )
        .merge_into(
            &CommonConfig::from_value(
                &jsonc::parse(&format!(r#"{{"postCreateCommand":"{config_command}"}}"#))
                    .unwrap()
                    .value(),
            )
            .unwrap(),
        );
        assert_eq!(
            merged.post_create_command,
            Some(Value::String(config_command))
        );
    }

    #[test]
    fn when_merge_into_with_unparsable_entry_then_returns_the_remaining_entries() {
        let user = random_word();
        let merged = Metadata::from(
            jsonc::parse(&format!(
                r#"[{{"capAdd":"{}"}},{{"remoteUser":"{user}"}}]"#,
                random_word()
            ))
            .unwrap()
            .value(),
        )
        .merge_into(&CommonConfig::from_value(&jsonc::parse("{}").unwrap().value()).unwrap());
        assert_eq!(merged.remote_user, Some(user));
    }

    #[test]
    fn when_lifecycle_commands_with_entries_then_returns_them_in_order() {
        let (first, last) = (random_word(), random_word());
        let metadata = Metadata::from(jsonc::parse(&format!(
            r#"[{{"postCreateCommand":"{first}"}},{{"id":"{}"}},{{"postCreateCommand":"{last}"}}]"#,
            random_word()
        )).unwrap().value());
        assert_eq!(
            metadata.lifecycle_commands("postCreateCommand"),
            vec![Value::String(first), Value::String(last)]
        );
    }

    #[test]
    fn when_lifecycle_commands_without_the_key_then_returns_empty() {
        let metadata = Metadata::from(
            jsonc::parse(&format!(r#"[{{"remoteUser":"{}"}}]"#, random_word()))
                .unwrap()
                .value(),
        );
        assert_eq!(metadata.lifecycle_commands("postCreateCommand"), vec![]);
    }

    #[test]
    fn when_mount_target_with_target_key_then_returns_its_value() {
        let target = random_word();
        for key in ["target", "dst", "destination"] {
            assert_eq!(
                mount_target(&format!("type=bind,src=/{},{key}=/{target}", random_word())),
                Some(format!("/{target}").as_str())
            );
        }
    }

    #[test]
    fn when_mount_target_without_target_key_then_returns_none() {
        assert_eq!(
            mount_target(&format!("type=volume,src=/{}", random_word())),
            None
        );
    }
}
