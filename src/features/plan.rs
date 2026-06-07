use super::manifest::Feature;
use anyhow::{Result, anyhow};
use serde_json::Value;
use std::collections::HashMap;

pub struct InstallPlan(Vec<Feature>);

impl InstallPlan {
    pub fn new(features: Vec<Feature>, override_order: &[String]) -> Result<Self> {
        let n = features.len();
        let id_to_idx: HashMap<&str, usize> = features
            .iter()
            .enumerate()
            .map(|(i, f)| (f.short_id.as_str(), i))
            .collect();

        let mut in_degree = vec![0usize; n];
        let mut adj: Vec<Vec<usize>> = vec![vec![]; n];

        for (i, f) in features.iter().enumerate() {
            for dep in &f.installs_after {
                let j = id_to_idx.get(dep.as_str()).or_else(|| {
                    dep.rsplit('/')
                        .next()
                        .and_then(|s| s.split_once(':').map(|(id, _)| id).or(Some(s)))
                        .and_then(|s| id_to_idx.get(s))
                });
                if let Some(&j) = j {
                    adj[j].push(i);
                    in_degree[i] += 1;
                }
            }
        }

        let priority: Vec<usize> = features
            .iter()
            .map(|f| {
                override_order
                    .iter()
                    .position(|id| {
                        id == &f.short_id
                            || id
                                .rsplit('/')
                                .next()
                                .and_then(|s| s.split_once(':').map(|(id, _)| id).or(Some(s)))
                                .is_some_and(|s| s == f.short_id)
                    })
                    .map_or(0, |pos| override_order.len() - pos)
            })
            .collect();

        let mut eligible: Vec<usize> = (0..n).filter(|&i| in_degree[i] == 0).collect();
        let mut order = Vec::with_capacity(n);
        while !eligible.is_empty() {
            let max_priority = eligible.iter().map(|&i| priority[i]).max().unwrap();
            let mut next_eligible = Vec::new();
            let mut round = Vec::new();
            for i in eligible {
                if priority[i] == max_priority {
                    round.push(i);
                } else {
                    next_eligible.push(i);
                }
            }
            for i in round {
                order.push(i);
                for &j in &adj[i] {
                    in_degree[j] -= 1;
                    if in_degree[j] == 0 {
                        next_eligible.push(j);
                    }
                }
            }
            eligible = next_eligible;
        }

        if order.len() != n {
            return Err(anyhow!("circular dependency detected in features"));
        }

        let mut slots: Vec<Option<Feature>> = features.into_iter().map(Some).collect();
        Ok(Self(
            order
                .into_iter()
                .map(|i| slots[i].take().unwrap())
                .collect(),
        ))
    }

    pub fn features(&self) -> &[Feature] {
        &self.0
    }
}

pub fn feature_dockerfile(base_content: &str, plan: &InstallPlan) -> String {
    let lines: Vec<String> = std::iter::once("USER root".to_string())
        .chain(plan.features().iter().flat_map(|feature| {
            let dir_name = feature
                .dir
                .file_name()
                .unwrap_or_default()
                .to_string_lossy();
            let dest = format!("/tmp/dev-container-features/{}", feature.short_id);
            let copy = format!("COPY ./{dir_name}/ {dest}/");
            let exports = match &feature.options {
                Value::Object(map) => map
                    .iter()
                    .map(|(k, v)| {
                        let val = match v {
                            Value::String(s) => s.clone(),
                            other => other.to_string(),
                        };
                        let env_key = k.to_uppercase().replace('-', "_");
                        let quoted = format!("'{}'", val.replace('\'', r"'\''"));
                        format!("export {env_key}={quoted}")
                    })
                    .collect::<Vec<_>>()
                    .join(" && "),
                _ => String::new(),
            };
            let run = if exports.is_empty() {
                format!(
                    "RUN chmod -R 0755 {dest} && cd {dest} && chmod +x ./install.sh && ./install.sh"
                )
            } else {
                format!(
                    "RUN {exports} && chmod -R 0755 {dest} && cd {dest} && chmod +x ./install.sh && ./install.sh"
                )
            };
            let mut steps = vec![copy, run];
            let mut env_keys: Vec<&String> = feature.container_env.keys().collect();
            env_keys.sort();
            for key in env_keys {
                let value = &feature.container_env[key];
                steps.push(format!("ENV {key}={value}"));
            }
            steps
        }))
        .collect();

    format!("{base_content}\n{}", lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::path::PathBuf;

    fn make_feature(short_id: &str, installs_after: Vec<&str>) -> Feature {
        Feature {
            short_id: short_id.to_string(),
            dir: PathBuf::from(format!("/{short_id}")),
            options: json!({}),
            installs_after: installs_after.into_iter().map(String::from).collect(),
            container_env: HashMap::new(),
            privileged: None,
            init: None,
            cap_add: vec![],
            mounts: vec![],
            on_create_command: None,
            update_content_command: None,
            post_create_command: None,
            post_start_command: None,
            post_attach_command: None,
        }
    }

    #[test]
    fn when_installs_after_declared_then_dependency_comes_first() {
        let features = vec![
            make_feature("node", vec!["common-utils"]),
            make_feature("common-utils", vec![]),
        ];
        let plan = InstallPlan::new(features, &[]).unwrap();
        let ids: Vec<_> = plan
            .features()
            .iter()
            .map(|f| f.short_id.as_str())
            .collect();
        assert_eq!(ids, vec!["common-utils", "node"]);
    }

    #[test]
    fn when_installs_after_references_oci_id_then_resolves_by_short_id() {
        let features = vec![
            make_feature(
                "node",
                vec!["ghcr.io/devcontainers/features/common-utils:1"],
            ),
            make_feature("common-utils", vec![]),
        ];
        let plan = InstallPlan::new(features, &[]).unwrap();
        let ids: Vec<_> = plan
            .features()
            .iter()
            .map(|f| f.short_id.as_str())
            .collect();
        assert_eq!(ids, vec!["common-utils", "node"]);
    }

    #[test]
    fn when_circular_dependency_then_returns_error() {
        let features = vec![make_feature("a", vec!["b"]), make_feature("b", vec!["a"])];
        assert!(InstallPlan::new(features, &[]).is_err());
    }

    #[test]
    fn when_installs_after_references_unknown_feature_then_ignored() {
        let features = vec![make_feature("git", vec!["unknown-feature"])];
        let plan = InstallPlan::new(features, &[]).unwrap();
        assert_eq!(plan.features().len(), 1);
    }

    #[test]
    fn when_override_order_specified_then_listed_features_install_first() {
        let features = vec![
            make_feature("a", vec![]),
            make_feature("b", vec![]),
            make_feature("c", vec![]),
        ];
        let plan = InstallPlan::new(features, &["c".to_string(), "b".to_string()]).unwrap();
        let ids: Vec<_> = plan
            .features()
            .iter()
            .map(|f| f.short_id.as_str())
            .collect();
        assert_eq!(ids, vec!["c", "b", "a"]);
    }

    #[test]
    fn when_override_order_conflicts_with_installs_after_then_installs_after_wins() {
        let features = vec![make_feature("a", vec![]), make_feature("b", vec!["a"])];
        let plan = InstallPlan::new(features, &["b".to_string()]).unwrap();
        let ids: Vec<_> = plan
            .features()
            .iter()
            .map(|f| f.short_id.as_str())
            .collect();
        assert_eq!(ids, vec!["a", "b"]);
    }

    #[test]
    fn when_override_order_uses_oci_ref_then_matches_by_short_id() {
        let features = vec![make_feature("a", vec![]), make_feature("b", vec![])];
        let plan = InstallPlan::new(
            features,
            &["ghcr.io/devcontainers/features/b:1".to_string()],
        )
        .unwrap();
        let ids: Vec<_> = plan
            .features()
            .iter()
            .map(|f| f.short_id.as_str())
            .collect();
        assert_eq!(ids, vec!["b", "a"]);
    }

    #[test]
    fn when_feature_dockerfile_with_no_options_then_runs_install_script() {
        let features = vec![Feature {
            short_id: "git".to_string(),
            dir: PathBuf::from("/tmp/0"),
            options: json!({}),
            installs_after: vec![],
            container_env: HashMap::new(),
            privileged: None,
            init: None,
            cap_add: vec![],
            mounts: vec![],
            on_create_command: None,
            update_content_command: None,
            post_create_command: None,
            post_start_command: None,
            post_attach_command: None,
        }];
        let plan = InstallPlan::new(features, &[]).unwrap();
        let df = feature_dockerfile("FROM rust:latest", &plan);
        assert!(df.contains("COPY ./0/ /tmp/dev-container-features/git/"));
        assert!(df.contains(
            "RUN chmod -R 0755 /tmp/dev-container-features/git && cd /tmp/dev-container-features/git && chmod +x ./install.sh && ./install.sh"
        ));
    }

    #[test]
    fn when_feature_dockerfile_with_string_option_then_exports_uppercased_key() {
        let features = vec![Feature {
            short_id: "node".to_string(),
            dir: PathBuf::from("/tmp/0"),
            options: json!({ "version": "18" }),
            installs_after: vec![],
            container_env: HashMap::new(),
            privileged: None,
            init: None,
            cap_add: vec![],
            mounts: vec![],
            on_create_command: None,
            update_content_command: None,
            post_create_command: None,
            post_start_command: None,
            post_attach_command: None,
        }];
        let plan = InstallPlan::new(features, &[]).unwrap();
        let df = feature_dockerfile("FROM ubuntu:22.04", &plan);
        assert!(df.contains("export VERSION='18'"));
    }

    #[test]
    fn when_feature_dockerfile_with_boolean_option_then_exports_serialized_value() {
        let features = vec![Feature {
            short_id: "docker".to_string(),
            dir: PathBuf::from("/tmp/0"),
            options: json!({ "moby": true }),
            installs_after: vec![],
            container_env: HashMap::new(),
            privileged: None,
            init: None,
            cap_add: vec![],
            mounts: vec![],
            on_create_command: None,
            update_content_command: None,
            post_create_command: None,
            post_start_command: None,
            post_attach_command: None,
        }];
        let plan = InstallPlan::new(features, &[]).unwrap();
        let df = feature_dockerfile("FROM ubuntu:22.04", &plan);
        assert!(df.contains("export MOBY='true'"));
    }

    #[test]
    fn when_feature_dockerfile_with_container_env_then_includes_env_directives() {
        let features = vec![Feature {
            short_id: "node".to_string(),
            dir: PathBuf::from("/tmp/0"),
            options: json!({}),
            installs_after: vec![],
            container_env: HashMap::from([("NVM_DIR".to_string(), "/usr/local/nvm".to_string())]),
            privileged: None,
            init: None,
            cap_add: vec![],
            mounts: vec![],
            on_create_command: None,
            update_content_command: None,
            post_create_command: None,
            post_start_command: None,
            post_attach_command: None,
        }];
        let plan = InstallPlan::new(features, &[]).unwrap();
        let df = feature_dockerfile("FROM ubuntu:22.04", &plan);
        assert!(df.contains("ENV NVM_DIR=/usr/local/nvm"));
    }

    #[test]
    fn when_feature_dockerfile_with_non_object_options_then_no_exports() {
        let features = vec![Feature {
            short_id: "git".to_string(),
            dir: PathBuf::from("/tmp/0"),
            options: json!(null),
            installs_after: vec![],
            container_env: HashMap::new(),
            privileged: None,
            init: None,
            cap_add: vec![],
            mounts: vec![],
            on_create_command: None,
            update_content_command: None,
            post_create_command: None,
            post_start_command: None,
            post_attach_command: None,
        }];
        let plan = InstallPlan::new(features, &[]).unwrap();
        let df = feature_dockerfile("FROM ubuntu:22.04", &plan);
        assert!(!df.contains("export"));
    }
}
