use anyhow::{Result, anyhow};
use serde::Deserialize;
use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::path::PathBuf;

pub enum FeatureSource {
    Local(PathBuf),
    Tarball(String),
    Oci {
        registry: String,
        path: String,
        version: String,
    },
}

impl FeatureSource {
    pub fn parse(id: &str) -> Result<Self> {
        if id.starts_with("./") || id.starts_with('/') {
            return Ok(Self::Local(PathBuf::from(id)));
        }
        if id.starts_with("https://") || id.starts_with("http://") {
            return Ok(Self::Tarball(id.to_string()));
        }
        let (without_version, version) = id
            .rsplit_once(':')
            .map(|(a, b)| (a, b.to_string()))
            .unwrap_or((id, "latest".to_string()));
        let slash = without_version
            .find('/')
            .ok_or_else(|| anyhow!("invalid OCI feature ref: {id}"))?;
        Ok(Self::Oci {
            registry: without_version[..slash].to_string(),
            path: without_version[slash + 1..].to_string(),
            version,
        })
    }
}

#[derive(Deserialize)]
pub struct FeatureManifest {
    pub id: String,
    #[serde(rename = "installsAfter", default)]
    pub installs_after: Vec<String>,
}

impl FeatureManifest {
    pub fn parse(content: &str) -> Result<Self> {
        serde_json::from_str(content)
            .map_err(|e| anyhow!("failed to parse devcontainer-feature.json: {e}"))
    }
}

pub struct Feature {
    pub short_id: String,
    pub dir: PathBuf,
    pub options: Value,
    pub installs_after: Vec<String>,
}

pub struct InstallPlan(Vec<Feature>);

impl InstallPlan {
    pub fn new(features: Vec<Feature>) -> Result<Self> {
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

        let mut queue: VecDeque<usize> = (0..n).filter(|&i| in_degree[i] == 0).collect();
        let mut order = Vec::with_capacity(n);
        while let Some(i) = queue.pop_front() {
            order.push(i);
            for &j in &adj[i] {
                in_degree[j] -= 1;
                if in_degree[j] == 0 {
                    queue.push_back(j);
                }
            }
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
            [copy, run]
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
        }
    }

    #[test]
    fn parse_remote_oci() {
        assert!(matches!(
            FeatureSource::parse("ghcr.io/devcontainers/features/git:1"),
            Ok(FeatureSource::Oci { .. })
        ));
    }

    #[test]
    fn parse_remote_tarball() {
        assert!(matches!(
            FeatureSource::parse("https://example.com/feature.tgz"),
            Ok(FeatureSource::Tarball(_))
        ));
    }

    #[test]
    fn parse_local() {
        assert!(matches!(
            FeatureSource::parse("./my-feature"),
            Ok(FeatureSource::Local(_))
        ));
    }

    #[test]
    fn sort_respects_installs_after() {
        let features = vec![
            make_feature("node", vec!["common-utils"]),
            make_feature("common-utils", vec![]),
        ];
        let plan = InstallPlan::new(features).unwrap();
        let ids: Vec<_> = plan
            .features()
            .iter()
            .map(|f| f.short_id.as_str())
            .collect();
        assert_eq!(ids, vec!["common-utils", "node"]);
    }

    #[test]
    fn sort_respects_installs_after_with_oci_version() {
        let features = vec![
            make_feature(
                "node",
                vec!["ghcr.io/devcontainers/features/common-utils:1"],
            ),
            make_feature("common-utils", vec![]),
        ];
        let plan = InstallPlan::new(features).unwrap();
        let ids: Vec<_> = plan
            .features()
            .iter()
            .map(|f| f.short_id.as_str())
            .collect();
        assert_eq!(ids, vec!["common-utils", "node"]);
    }

    #[test]
    fn sort_detects_cycle() {
        let features = vec![make_feature("a", vec!["b"]), make_feature("b", vec!["a"])];
        assert!(InstallPlan::new(features).is_err());
    }

    #[test]
    fn sort_unknown_dep_is_ignored() {
        let features = vec![make_feature("git", vec!["unknown-feature"])];
        let plan = InstallPlan::new(features).unwrap();
        assert_eq!(plan.features().len(), 1);
    }

    #[test]
    fn feature_dockerfile_no_options() {
        let features = vec![Feature {
            short_id: "git".to_string(),
            dir: PathBuf::from("/tmp/0"),
            options: json!({}),
            installs_after: vec![],
        }];
        let plan = InstallPlan::new(features).unwrap();
        let df = feature_dockerfile("FROM rust:latest", &plan);
        assert!(df.contains("FROM rust:latest"));
        assert!(df.contains("COPY ./0/ /tmp/dev-container-features/git/"));
        assert!(df.contains("./install.sh"));
    }

    #[test]
    fn feature_dockerfile_with_options() {
        let features = vec![Feature {
            short_id: "node".to_string(),
            dir: PathBuf::from("/tmp/0"),
            options: json!({ "version": "18" }),
            installs_after: vec![],
        }];
        let plan = InstallPlan::new(features).unwrap();
        let df = feature_dockerfile("FROM ubuntu:22.04", &plan);
        assert!(df.contains("export VERSION='18'"));
    }

    #[test]
    fn feature_manifest_parse() {
        let content = r#"{"id":"git","version":"1.0","installsAfter":["common-utils"]}"#;
        let m = FeatureManifest::parse(content).unwrap();
        assert_eq!(m.id, "git");
        assert_eq!(m.installs_after, vec!["common-utils"]);
    }
}
