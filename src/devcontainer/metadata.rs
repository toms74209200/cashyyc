use super::config::CommonConfig;
use super::jsonc::Value;
use crate::lifecycle::LifecycleCmd;

#[derive(Debug, Clone, PartialEq)]
pub struct Metadata(Vec<Value>);

#[derive(Debug, PartialEq)]
pub struct Merged {
    pub config: CommonConfig,
    pub entrypoints: Vec<String>,
    pub on_create_commands: Vec<LifecycleCmd>,
    pub update_content_commands: Vec<LifecycleCmd>,
    pub post_create_commands: Vec<LifecycleCmd>,
    pub post_start_commands: Vec<LifecycleCmd>,
    pub post_attach_commands: Vec<LifecycleCmd>,
}

const UPDATEABLE_PROPERTIES: &[&str] = &["remoteUser", "userEnvProbe", "remoteEnv"];

impl Metadata {
    pub const LABEL: &str = "devcontainer.metadata";

    pub fn for_existing_container(label: Option<&str>, config: &CommonConfig) -> Self {
        let label: Vec<Value> = match label
            .and_then(|label| super::jsonc::parse(label.trim()).ok())
            .map(|document| document.value())
        {
            Some(Value::Array(entries)) => entries,
            Some(entry @ Value::Object(_)) => vec![entry],
            _ => vec![],
        }
        .into_iter()
        .filter(|entry| !matches!(entry, Value::Object(members) if members.is_empty()))
        .collect();
        let config_entry: Vec<(String, Value)> = config
            .metadata
            .iter()
            .filter(|(key, _)| label.is_empty() || UPDATEABLE_PROPERTIES.contains(&key.as_str()))
            .cloned()
            .collect();
        Self(
            label
                .into_iter()
                .chain((!config_entry.is_empty()).then_some(Value::Object(config_entry)))
                .collect(),
        )
    }

    pub fn for_container<'a>(
        image_label: Option<&str>,
        features: impl IntoIterator<Item = &'a Vec<(String, Value)>>,
        config: &'a CommonConfig,
    ) -> Self {
        Self(
            match image_label
                .and_then(|label| super::jsonc::parse(label.trim()).ok())
                .map(|document| document.value())
            {
                Some(Value::Array(entries)) => entries,
                Some(entry @ Value::Object(_)) => vec![entry],
                _ => vec![],
            }
            .into_iter()
            .chain(
                features
                    .into_iter()
                    .chain([&config.metadata])
                    .map(|members| Value::Object(members.clone())),
            )
            .filter(|entry| !matches!(entry, Value::Object(members) if members.is_empty()))
            .collect(),
        )
    }

    pub fn label(&self) -> String {
        format!("{}={}", Self::LABEL, Value::Array(self.0.clone()))
    }

    pub fn merge(&self, config: &CommonConfig) -> Merged {
        let commands = |key: &str| -> Vec<LifecycleCmd> {
            self.0
                .iter()
                .filter_map(|entry| LifecycleCmd::try_from(entry.get(key)?).ok())
                .collect()
        };
        Merged {
            config: self.0.iter().filter_map(CommonConfig::from_value).fold(
                CommonConfig {
                    init: None,
                    privileged: None,
                    cap_add: vec![],
                    security_opt: vec![],
                    mounts: vec![],
                    container_env: Default::default(),
                    remote_env: None,
                    container_user: None,
                    remote_user: None,
                    update_remote_user_uid: None,
                    user_env_probe: None,
                    override_command: None,
                    wait_for: None,
                    ..config.clone()
                },
                |earlier, later| CommonConfig {
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
                        |merged, cap| match merged.contains(cap) {
                            true => merged,
                            false => [merged, vec![cap.clone()]].concat(),
                        },
                    ),
                    security_opt: earlier.security_opt.iter().chain(&later.security_opt).fold(
                        vec![],
                        |merged, opt| match merged.contains(opt) {
                            true => merged,
                            false => [merged, vec![opt.clone()]].concat(),
                        },
                    ),
                    mounts: {
                        let mounts: Vec<_> = earlier.mounts.iter().chain(&later.mounts).collect();
                        mounts
                            .iter()
                            .enumerate()
                            .filter(|(i, m)| !mounts[i + 1..].iter().any(|n| n.target == m.target))
                            .map(|(_, m)| (*m).clone())
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
                    ..earlier
                },
            ),
            entrypoints: self
                .0
                .iter()
                .filter_map(|entry| entry.get("entrypoint")?.as_str())
                .map(str::to_string)
                .collect(),
            on_create_commands: commands("onCreateCommand"),
            update_content_commands: commands("updateContentCommand"),
            post_create_commands: commands("postCreateCommand"),
            post_start_commands: commands("postStartCommand"),
            post_attach_commands: commands("postAttachCommand"),
        }
    }
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
    fn when_for_container_then_label_has_image_features_config_in_order() {
        let (base, feature_a, feature_b, user) =
            (random_word(), random_word(), random_word(), random_word());
        let features = [
            vec![("id".to_string(), Value::String(feature_a.clone()))],
            vec![("id".to_string(), Value::String(feature_b.clone()))],
        ];
        let config = CommonConfig::from_value(
            &jsonc::parse(&format!(r#"{{"remoteUser":"{user}"}}"#))
                .unwrap()
                .value(),
        )
        .unwrap();

        let metadata = Metadata::for_container(
            Some(&format!(r#"[{{"remoteUser":"{base}"}}]"#)),
            &features,
            &config,
        );

        assert_eq!(
            metadata.label(),
            format!(
                r#"devcontainer.metadata=[{{"remoteUser":"{base}"}},{{"id":"{feature_a}"}},{{"id":"{feature_b}"}},{{"remoteUser":"{user}"}}]"#
            )
        );
    }

    #[test]
    fn when_for_container_with_empty_parts_then_label_has_empty_array() {
        let config = CommonConfig::from_value(&jsonc::parse("{}").unwrap().value()).unwrap();

        let metadata = Metadata::for_container(None, [], &config);

        assert_eq!(metadata.label(), "devcontainer.metadata=[]");
    }

    #[test]
    fn when_for_container_with_empty_entries_then_label_has_no_empty_entries() {
        let user = random_word();
        let config = CommonConfig::from_value(&jsonc::parse("{}").unwrap().value()).unwrap();

        let metadata = Metadata::for_container(
            Some(&format!(r#"[{{}},{{"remoteUser":"{user}"}},{{}}]"#)),
            &[vec![]],
            &config,
        );

        assert_eq!(
            metadata.label(),
            format!(r#"devcontainer.metadata=[{{"remoteUser":"{user}"}}]"#)
        );
    }

    #[test]
    fn when_for_container_with_object_image_label_then_label_has_it_as_one_entry() {
        let user = random_word();
        let config = CommonConfig::from_value(&jsonc::parse("{}").unwrap().value()).unwrap();

        let metadata =
            Metadata::for_container(Some(&format!(r#"{{"remoteUser":"{user}"}}"#)), [], &config);

        assert_eq!(
            metadata.label(),
            format!(r#"devcontainer.metadata=[{{"remoteUser":"{user}"}}]"#)
        );
    }

    #[test]
    fn when_for_container_with_invalid_image_label_then_label_has_the_config_only() {
        let user = random_word();
        let config = CommonConfig::from_value(
            &jsonc::parse(&format!(r#"{{"remoteUser":"{user}"}}"#))
                .unwrap()
                .value(),
        )
        .unwrap();

        let metadata = Metadata::for_container(Some("<no value>"), [], &config);

        assert_eq!(
            metadata.label(),
            format!(r#"devcontainer.metadata=[{{"remoteUser":"{user}"}}]"#)
        );
    }

    #[test]
    fn when_for_container_with_image_label_with_trailing_newline_then_label_has_its_entries() {
        let user = random_word();
        let config = CommonConfig::from_value(&jsonc::parse("{}").unwrap().value()).unwrap();

        let metadata = Metadata::for_container(
            Some(&format!("[{{\"remoteUser\":\"{user}\"}}]\n")),
            [],
            &config,
        );

        assert_eq!(
            metadata.label(),
            format!(r#"devcontainer.metadata=[{{"remoteUser":"{user}"}}]"#)
        );
    }

    #[test]
    fn when_for_container_with_spaced_image_label_then_label_has_compact_json() {
        let user = random_word();
        let config = CommonConfig::from_value(&jsonc::parse("{}").unwrap().value()).unwrap();

        let metadata = Metadata::for_container(
            Some(&format!(r#"[{{ "remoteUser" : "{user}" }}]"#)),
            [],
            &config,
        );

        assert_eq!(
            metadata.label(),
            format!(r#"devcontainer.metadata=[{{"remoteUser":"{user}"}}]"#)
        );
    }

    #[test]
    fn when_merge_with_config_only_then_returns_the_config_as_is() {
        let config = CommonConfig::from_value(
            &jsonc::parse(&format!(
                r#"{{"remoteUser":"{}","capAdd":["{}"],"init":true}}"#,
                random_word(),
                random_word()
            ))
            .unwrap()
            .value(),
        )
        .unwrap();

        let merged = Metadata::for_container(None, [], &config).merge(&config);

        assert_eq!(merged.config, config);
    }

    #[test]
    fn when_merge_with_privileged_feature_entry_then_returns_privileged() {
        let features = [vec![
            ("id".to_string(), Value::String(random_word())),
            ("privileged".to_string(), Value::Bool(true)),
        ]];
        let config = CommonConfig::from_value(&jsonc::parse("{}").unwrap().value()).unwrap();

        let merged = Metadata::for_container(None, &features, &config).merge(&config);

        assert_eq!(merged.config.privileged, Some(true));
    }

    #[test]
    fn when_merge_with_object_mount_in_feature_entry_then_returns_the_mount_as_string() {
        let (source, target) = (format!("/{}", random_word()), format!("/{}", random_word()));
        let features = [jsonc::parse(&format!(
            r#"{{"id":"{}","mounts":[{{"type":"bind","source":"{source}","target":"{target}"}}]}}"#,
            random_word()
        ))
        .unwrap()
        .value()
        .as_object()
        .unwrap()
        .to_vec()];
        let config = CommonConfig::from_value(&jsonc::parse("{}").unwrap().value()).unwrap();

        let merged = Metadata::for_container(None, &features, &config).merge(&config);

        assert_eq!(
            merged
                .config
                .mounts
                .into_iter()
                .map(|m| m.spec)
                .collect::<Vec<_>>(),
            vec![format!("type=bind,src={source},dst={target}")]
        );
    }

    #[test]
    fn when_merge_with_remote_user_only_in_image_then_returns_the_image_user() {
        let user = random_word();
        let config = CommonConfig::from_value(&jsonc::parse("{}").unwrap().value()).unwrap();

        let merged = Metadata::for_container(
            Some(&format!(r#"[{{"remoteUser":"{user}"}}]"#)),
            [],
            &config,
        )
        .merge(&config);

        assert_eq!(merged.config.remote_user, Some(user));
    }

    #[test]
    fn when_merge_with_remote_user_in_both_then_returns_the_config_user() {
        let (image_user, config_user) = (random_word(), random_word());
        let config = CommonConfig::from_value(
            &jsonc::parse(&format!(r#"{{"remoteUser":"{config_user}"}}"#))
                .unwrap()
                .value(),
        )
        .unwrap();

        let merged = Metadata::for_container(
            Some(&format!(r#"[{{"remoteUser":"{image_user}"}}]"#)),
            [],
            &config,
        )
        .merge(&config);

        assert_eq!(merged.config.remote_user, Some(config_user));
    }

    #[test]
    fn when_merge_with_multiple_entries_then_returns_the_last_value() {
        let (first, last) = (random_word(), random_word());
        let config = CommonConfig::from_value(&jsonc::parse("{}").unwrap().value()).unwrap();

        let merged = Metadata::for_container(
            Some(&format!(
                r#"[{{"remoteUser":"{first}"}},{{"remoteUser":"{last}"}}]"#
            )),
            [],
            &config,
        )
        .merge(&config);

        assert_eq!(merged.config.remote_user, Some(last));
    }

    #[test]
    fn when_merge_with_init_true_in_image_then_returns_init_true() {
        let config = CommonConfig::from_value(&jsonc::parse("{}").unwrap().value()).unwrap();

        let merged =
            Metadata::for_container(Some(r#"[{"init":true}]"#), [], &config).merge(&config);

        assert_eq!(merged.config.init, Some(true));
    }

    #[test]
    fn when_merge_with_privileged_true_in_image_and_false_in_config_then_returns_privileged_true() {
        let config =
            CommonConfig::from_value(&jsonc::parse(r#"{"privileged":false}"#).unwrap().value())
                .unwrap();

        let merged =
            Metadata::for_container(Some(r#"[{"privileged":true}]"#), [], &config).merge(&config);

        assert_eq!(merged.config.privileged, Some(true));
    }

    #[test]
    fn when_merge_with_init_false_in_both_then_returns_init_false() {
        let config =
            CommonConfig::from_value(&jsonc::parse(r#"{"init":false}"#).unwrap().value()).unwrap();

        let merged =
            Metadata::for_container(Some(r#"[{"init":false}]"#), [], &config).merge(&config);

        assert_eq!(merged.config.init, Some(false));
    }

    #[test]
    fn when_merge_with_cap_add_in_both_then_returns_the_union_without_duplicates() {
        let (shared, image_only, config_only) = (random_word(), random_word(), random_word());
        let config = CommonConfig::from_value(
            &jsonc::parse(&format!(r#"{{"capAdd":["{shared}","{config_only}"]}}"#))
                .unwrap()
                .value(),
        )
        .unwrap();

        let merged = Metadata::for_container(
            Some(&format!(r#"[{{"capAdd":["{shared}","{image_only}"]}}]"#)),
            [],
            &config,
        )
        .merge(&config);

        assert_eq!(merged.config.cap_add, vec![shared, image_only, config_only]);
    }

    #[test]
    fn when_merge_with_security_opt_in_image_only_then_returns_it() {
        let opt = random_word();
        let config = CommonConfig::from_value(&jsonc::parse("{}").unwrap().value()).unwrap();

        let merged = Metadata::for_container(
            Some(&format!(r#"[{{"securityOpt":["{opt}"]}}]"#)),
            [],
            &config,
        )
        .merge(&config);

        assert_eq!(merged.config.security_opt, vec![opt]);
    }

    #[test]
    fn when_merge_with_container_env_of_the_same_key_then_returns_the_config_value() {
        let (key, image_value, config_value) = (random_word(), random_word(), random_word());
        let config = CommonConfig::from_value(
            &jsonc::parse(&format!(
                r#"{{"containerEnv":{{"{key}":"{config_value}"}}}}"#
            ))
            .unwrap()
            .value(),
        )
        .unwrap();

        let merged = Metadata::for_container(
            Some(&format!(
                r#"[{{"containerEnv":{{"{key}":"{image_value}"}}}}]"#
            )),
            [],
            &config,
        )
        .merge(&config);

        assert_eq!(merged.config.container_env.get(&key), Some(&config_value));
    }

    #[test]
    fn when_merge_with_container_env_of_different_keys_then_returns_both() {
        let (image_key, config_key, value) = (random_word(), random_word(), random_word());
        let config = CommonConfig::from_value(
            &jsonc::parse(&format!(
                r#"{{"containerEnv":{{"{config_key}":"{value}"}}}}"#
            ))
            .unwrap()
            .value(),
        )
        .unwrap();

        let merged = Metadata::for_container(
            Some(&format!(
                r#"[{{"containerEnv":{{"{image_key}":"{value}"}}}}]"#
            )),
            [],
            &config,
        )
        .merge(&config);

        assert_eq!(merged.config.container_env.get(&image_key), Some(&value));
        assert_eq!(merged.config.container_env.get(&config_key), Some(&value));
    }

    #[test]
    fn when_merge_with_mounts_of_the_same_target_then_returns_the_config_mount_only() {
        let (target, image_source, config_source) = (random_word(), random_word(), random_word());
        let config = CommonConfig::from_value(
            &jsonc::parse(&format!(
                r#"{{"mounts":["type=bind,src=/{config_source},dst=/{target}"]}}"#
            ))
            .unwrap()
            .value(),
        )
        .unwrap();

        let merged = Metadata::for_container(
            Some(&format!(
                r#"[{{"mounts":["type=bind,src=/{image_source},target=/{target}"]}}]"#
            )),
            [],
            &config,
        )
        .merge(&config);

        assert_eq!(
            merged
                .config
                .mounts
                .into_iter()
                .map(|m| m.spec)
                .collect::<Vec<_>>(),
            vec![format!("type=bind,src=/{config_source},dst=/{target}")]
        );
    }

    #[test]
    fn when_merge_with_mounts_of_the_same_target_in_one_entry_then_returns_the_later_only() {
        let (target, first, last) = (random_word(), random_word(), random_word());
        let config = CommonConfig::from_value(&jsonc::parse("{}").unwrap().value()).unwrap();

        let merged = Metadata::for_container(
            Some(&format!(
                r#"[{{"mounts":["type=bind,src=/{first},dst=/{target}","type=bind,src=/{last},destination=/{target}"]}}]"#
            )),
            [],
            &config,
        )
        .merge(&config);

        assert_eq!(
            merged
                .config
                .mounts
                .into_iter()
                .map(|m| m.spec)
                .collect::<Vec<_>>(),
            vec![format!("type=bind,src=/{last},destination=/{target}")]
        );
    }

    #[test]
    fn when_merge_with_mounts_of_different_targets_then_returns_the_image_mount_first() {
        let (image_target, config_target, source) = (random_word(), random_word(), random_word());
        let config = CommonConfig::from_value(
            &jsonc::parse(&format!(
                r#"{{"mounts":["type=bind,src=/{source},dst=/{config_target}"]}}"#
            ))
            .unwrap()
            .value(),
        )
        .unwrap();

        let merged = Metadata::for_container(
            Some(&format!(
                r#"[{{"mounts":["type=bind,src=/{source},dst=/{image_target}"]}}]"#
            )),
            [],
            &config,
        )
        .merge(&config);

        assert_eq!(
            merged
                .config
                .mounts
                .into_iter()
                .map(|m| m.spec)
                .collect::<Vec<_>>(),
            vec![
                format!("type=bind,src=/{source},dst=/{image_target}"),
                format!("type=bind,src=/{source},dst=/{config_target}"),
            ]
        );
    }

    #[test]
    fn when_merge_with_remote_env_in_both_then_returns_them_merged() {
        let (image_key, config_key, value) = (random_word(), random_word(), random_word());
        let config = CommonConfig::from_value(
            &jsonc::parse(&format!(r#"{{"remoteEnv":{{"{config_key}":"{value}"}}}}"#))
                .unwrap()
                .value(),
        )
        .unwrap();

        let merged = Metadata::for_container(
            Some(&format!(r#"[{{"remoteEnv":{{"{image_key}":"{value}"}}}}]"#)),
            [],
            &config,
        )
        .merge(&config);

        let remote_env = merged.config.remote_env.unwrap();
        assert_eq!(remote_env.get(&image_key), Some(&Some(value.clone())));
        assert_eq!(remote_env.get(&config_key), Some(&Some(value)));
    }

    #[test]
    fn when_merge_with_wait_for_only_in_image_then_returns_the_image_wait_for() {
        let config = CommonConfig::from_value(&jsonc::parse("{}").unwrap().value()).unwrap();

        let merged =
            Metadata::for_container(Some(r#"[{"waitFor":"onCreateCommand"}]"#), [], &config)
                .merge(&config);

        assert_eq!(merged.config.wait_for, Some(WaitFor::OnCreateCommand));
    }

    #[test]
    fn when_merge_with_user_env_probe_only_in_image_then_returns_the_image_probe() {
        let config = CommonConfig::from_value(&jsonc::parse("{}").unwrap().value()).unwrap();

        let merged =
            Metadata::for_container(Some(r#"[{"userEnvProbe":"loginShell"}]"#), [], &config)
                .merge(&config);

        assert_eq!(merged.config.user_env_probe, Some(UserEnvProbe::LoginShell));
    }

    #[test]
    fn when_merge_with_post_create_command_in_image_then_returns_the_config_command() {
        let (image_command, config_command) = (random_word(), random_word());
        let config = CommonConfig::from_value(
            &jsonc::parse(&format!(r#"{{"postCreateCommand":"{config_command}"}}"#))
                .unwrap()
                .value(),
        )
        .unwrap();

        let merged = Metadata::for_container(
            Some(&format!(r#"[{{"postCreateCommand":"{image_command}"}}]"#)),
            [],
            &config,
        )
        .merge(&config);

        assert_eq!(
            merged.config.post_create_command,
            Some(Value::String(config_command))
        );
    }

    #[test]
    fn when_merge_with_unparsable_entry_then_returns_the_remaining_entries() {
        let user = random_word();
        let config = CommonConfig::from_value(&jsonc::parse("{}").unwrap().value()).unwrap();

        let merged = Metadata::for_container(
            Some(&format!(
                r#"[{{"capAdd":"{}"}},{{"remoteUser":"{user}"}}]"#,
                random_word()
            )),
            [],
            &config,
        )
        .merge(&config);

        assert_eq!(merged.config.remote_user, Some(user));
    }

    #[test]
    fn when_merge_with_lifecycle_commands_then_returns_them_in_order() {
        let (image_command, feature_command, config_command) =
            (random_word(), random_word(), random_word());
        let features = [vec![(
            "postCreateCommand".to_string(),
            Value::Array(vec![Value::String(feature_command.clone())]),
        )]];
        let config = CommonConfig::from_value(
            &jsonc::parse(&format!(r#"{{"postCreateCommand":"{config_command}"}}"#))
                .unwrap()
                .value(),
        )
        .unwrap();

        let merged = Metadata::for_container(
            Some(&format!(r#"[{{"postCreateCommand":"{image_command}"}}]"#)),
            &features,
            &config,
        )
        .merge(&config);

        assert_eq!(
            merged.post_create_commands,
            vec![
                LifecycleCmd::Shell(image_command),
                LifecycleCmd::Exec(vec![feature_command]),
                LifecycleCmd::Shell(config_command),
            ]
        );
    }

    #[test]
    fn when_merge_without_lifecycle_commands_then_returns_no_commands() {
        let config = CommonConfig::from_value(
            &jsonc::parse(&format!(r#"{{"remoteUser":"{}"}}"#, random_word()))
                .unwrap()
                .value(),
        )
        .unwrap();

        let merged = Metadata::for_container(None, [], &config).merge(&config);

        assert_eq!(
            [
                merged.on_create_commands,
                merged.update_content_commands,
                merged.post_create_commands,
                merged.post_start_commands,
                merged.post_attach_commands,
            ],
            [vec![], vec![], vec![], vec![], vec![]]
        );
    }

    #[test]
    fn when_merge_with_entrypoints_then_returns_them_in_order() {
        let (first, last) = (random_word(), random_word());
        let config = CommonConfig::from_value(&jsonc::parse("{}").unwrap().value()).unwrap();

        let merged = Metadata::for_container(
            Some(&format!(
                r#"[{{"entrypoint":"/{first}"}},{{"id":"{}"}},{{"entrypoint":"/{last}"}}]"#,
                random_word()
            )),
            [],
            &config,
        )
        .merge(&config);

        assert_eq!(
            merged.entrypoints,
            vec![format!("/{first}"), format!("/{last}")]
        );
    }

    #[test]
    fn when_merge_without_entrypoints_then_returns_no_entrypoints() {
        let config = CommonConfig::from_value(
            &jsonc::parse(&format!(r#"{{"remoteUser":"{}"}}"#, random_word()))
                .unwrap()
                .value(),
        )
        .unwrap();

        let merged = Metadata::for_container(None, [], &config).merge(&config);

        assert!(merged.entrypoints.is_empty());
    }

    #[test]
    fn when_for_existing_container_with_label_then_label_has_the_label_followed_by_the_config_remote_user()
     {
        let (command, user) = (random_word(), random_word());
        let config = CommonConfig::from_value(
            &jsonc::parse(&format!(r#"{{"remoteUser":"{user}"}}"#))
                .unwrap()
                .value(),
        )
        .unwrap();

        let metadata = Metadata::for_existing_container(
            Some(&format!(r#"[{{"postAttachCommand":"{command}"}}]"#)),
            &config,
        );

        assert_eq!(
            metadata.label(),
            format!(
                r#"devcontainer.metadata=[{{"postAttachCommand":"{command}"}},{{"remoteUser":"{user}"}}]"#
            )
        );
    }

    #[test]
    fn when_for_existing_container_with_lifecycle_command_in_config_then_label_has_the_label_only()
    {
        let user = random_word();
        let config = CommonConfig::from_value(
            &jsonc::parse(&format!(r#"{{"postAttachCommand":"{}"}}"#, random_word()))
                .unwrap()
                .value(),
        )
        .unwrap();

        let metadata = Metadata::for_existing_container(
            Some(&format!(r#"[{{"remoteUser":"{user}"}}]"#)),
            &config,
        );

        assert_eq!(
            metadata.label(),
            format!(r#"devcontainer.metadata=[{{"remoteUser":"{user}"}}]"#)
        );
    }

    #[test]
    fn when_for_existing_container_without_label_then_label_has_the_config() {
        let command = random_word();
        let config = CommonConfig::from_value(
            &jsonc::parse(&format!(r#"{{"postAttachCommand":"{command}"}}"#))
                .unwrap()
                .value(),
        )
        .unwrap();

        for label in [None, Some(""), Some("<no value>"), Some("[{}]")] {
            let metadata = Metadata::for_existing_container(label, &config);

            assert_eq!(
                metadata.label(),
                format!(r#"devcontainer.metadata=[{{"postAttachCommand":"{command}"}}]"#)
            );
        }
    }

    #[test]
    fn when_merge_for_existing_container_with_container_env_only_in_config_then_returns_the_label_env()
     {
        let (key, label_value) = (random_word(), random_word());
        let config = CommonConfig::from_value(
            &jsonc::parse(&format!(
                r#"{{"containerEnv":{{"{key}":"{}"}}}}"#,
                random_word()
            ))
            .unwrap()
            .value(),
        )
        .unwrap();

        let merged = Metadata::for_existing_container(
            Some(&format!(
                r#"[{{"containerEnv":{{"{key}":"{label_value}"}}}}]"#
            )),
            &config,
        )
        .merge(&config);

        assert_eq!(merged.config.container_env.get(&key), Some(&label_value));
    }
}
