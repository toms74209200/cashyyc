use anyhow::{Result, anyhow};
use serde::Deserialize;
use serde_json::Value;
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};

pub enum FeatureSource {
    Local(PathBuf),
    Remote(String),
}

impl FeatureSource {
    pub fn parse(id: &str) -> Self {
        if id.starts_with("./") || id.starts_with('/') {
            Self::Local(PathBuf::from(id))
        } else {
            Self::Remote(id.to_string())
        }
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

pub fn feature_dockerfile(base_image: &str, plan: &InstallPlan) -> String {
    let mut lines = vec![format!("FROM {base_image}"), "USER root".to_string()];

    for feature in plan.features() {
        let dir_name = feature
            .dir
            .file_name()
            .unwrap_or_default()
            .to_string_lossy();
        let dest = format!("/tmp/dev-container-features/{}", feature.short_id);
        lines.push(format!("COPY ./{dir_name}/ {dest}/"));
        let exports = options_as_exports(&feature.options);
        let run = if exports.is_empty() {
            format!(
                "RUN chmod -R 0755 {dest} && cd {dest} && chmod +x ./install.sh && ./install.sh"
            )
        } else {
            format!(
                "RUN {exports} && chmod -R 0755 {dest} && cd {dest} && chmod +x ./install.sh && ./install.sh"
            )
        };
        lines.push(run);
    }

    lines.join("\n")
}

fn options_as_exports(options: &Value) -> String {
    let Value::Object(map) = options else {
        return String::new();
    };
    map.iter()
        .map(|(k, v)| {
            let val = match v {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            format!("export {}={}", k.to_uppercase(), shell_quote(&val))
        })
        .collect::<Vec<_>>()
        .join(" && ")
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

pub fn prepare(
    base_image: &str,
    features_map: &HashMap<String, Value>,
    devcontainer_dir: &Path,
    out_dir: &Path,
) -> Result<Option<PathBuf>> {
    if features_map.is_empty() {
        return Ok(None);
    }
    let mut resolved = Vec::new();
    for (idx, (id, options)) in features_map.iter().enumerate() {
        let source = match FeatureSource::parse(id) {
            FeatureSource::Local(p) if p.is_relative() => {
                FeatureSource::Local(devcontainer_dir.join(p))
            }
            other => other,
        };
        let feature_dir = out_dir.join(idx.to_string());
        std::fs::create_dir_all(&feature_dir)
            .map_err(|e| anyhow!("failed to create feature dir: {e}"))?;
        download_feature(&source, &feature_dir)?;
        let manifest_content =
            std::fs::read_to_string(feature_dir.join("devcontainer-feature.json"))
                .map_err(|e| anyhow!("devcontainer-feature.json not found in feature {id}: {e}"))?;
        let manifest = FeatureManifest::parse(&manifest_content)?;
        resolved.push(Feature {
            short_id: manifest.id,
            dir: feature_dir,
            options: options.clone(),
            installs_after: manifest.installs_after,
        });
    }
    let plan = InstallPlan::new(resolved)?;
    let dockerfile_content = feature_dockerfile(base_image, &plan);
    let dockerfile_path = out_dir.join("Dockerfile.features");
    std::fs::write(&dockerfile_path, &dockerfile_content)
        .map_err(|e| anyhow!("failed to write feature Dockerfile: {e}"))?;
    Ok(Some(dockerfile_path))
}

pub fn download_feature(source: &FeatureSource, dest_dir: &Path) -> Result<()> {
    match source {
        FeatureSource::Local(path) => {
            let status = std::process::Command::new("cp")
                .args([
                    "-r",
                    &format!("{}/.", path.display()),
                    &dest_dir.display().to_string(),
                ])
                .status()
                .map_err(|e| anyhow!("failed to copy local feature: {e}"))?;
            if !status.success() {
                return Err(anyhow!(
                    "failed to copy local feature from {}",
                    path.display()
                ));
            }
        }
        FeatureSource::Remote(id) => {
            let tarball = dest_dir.join("feature.tgz");
            if id.starts_with("https://") || id.starts_with("http://") {
                let status = std::process::Command::new("curl")
                    .args(["-sfL", id, "-o", &tarball.display().to_string()])
                    .status()
                    .map_err(|e| anyhow!("failed to run curl: {e}"))?;
                if !status.success() {
                    return Err(anyhow!("failed to download feature from {id}"));
                }
            } else {
                let (without_version, version) = id
                    .rsplit_once(':')
                    .map(|(a, b)| (a, b.to_string()))
                    .unwrap_or((id.as_str(), "latest".to_string()));
                let slash = without_version
                    .find('/')
                    .ok_or_else(|| anyhow!("invalid OCI feature ref: {id}"))?;
                let registry = &without_version[..slash];
                let path = &without_version[slash + 1..];
                let token = fetch_oci_token(registry, path)?;
                fetch_oci_blob(registry, path, &version, &token, &tarball)?;
            }
            extract_tarball(&tarball, dest_dir)?;
        }
    }
    Ok(())
}

fn fetch_oci_token(registry: &str, path: &str) -> Result<String> {
    let url = format!("https://{registry}/token?scope=repository:{path}:pull&service={registry}");
    let output = std::process::Command::new("curl")
        .args(["-sf", &url])
        .output()
        .map_err(|e| anyhow!("failed to run curl: {e}"))?;
    if !output.status.success() {
        return Err(anyhow!("failed to fetch OCI token for {registry}/{path}"));
    }
    let json: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| anyhow!("failed to parse OCI token response: {e}"))?;
    json["token"]
        .as_str()
        .map(String::from)
        .ok_or_else(|| anyhow!("OCI token response missing 'token' field"))
}

fn fetch_oci_blob(
    registry: &str,
    path: &str,
    version: &str,
    token: &str,
    dest: &Path,
) -> Result<()> {
    let manifest_url = format!("https://{registry}/v2/{path}/manifests/{version}");
    let output = std::process::Command::new("curl")
        .args([
            "-sf",
            "-H",
            &format!("Authorization: Bearer {token}"),
            "-H",
            "Accept: application/vnd.oci.image.manifest.v1+json",
            &manifest_url,
        ])
        .output()
        .map_err(|e| anyhow!("failed to run curl: {e}"))?;
    if !output.status.success() {
        return Err(anyhow!(
            "failed to fetch OCI manifest for {registry}/{path}:{version}"
        ));
    }
    let manifest: serde_json::Value = serde_json::from_slice(&output.stdout)
        .map_err(|e| anyhow!("failed to parse OCI manifest: {e}"))?;
    let digest = manifest["layers"][0]["digest"]
        .as_str()
        .ok_or_else(|| anyhow!("OCI manifest missing layers[0].digest"))?;

    let blob_url = format!("https://{registry}/v2/{path}/blobs/{digest}");
    let status = std::process::Command::new("curl")
        .args([
            "-sfL",
            "-H",
            &format!("Authorization: Bearer {token}"),
            "-o",
            &dest.display().to_string(),
            &blob_url,
        ])
        .status()
        .map_err(|e| anyhow!("failed to run curl: {e}"))?;
    if !status.success() {
        return Err(anyhow!("failed to download OCI blob for {registry}/{path}"));
    }
    Ok(())
}

fn extract_tarball(tarball: &Path, dest_dir: &Path) -> Result<()> {
    let status = std::process::Command::new("tar")
        .args([
            "xzf",
            &tarball.display().to_string(),
            "-C",
            &dest_dir.display().to_string(),
        ])
        .status()
        .map_err(|e| anyhow!("failed to run tar: {e}"))?;
    if !status.success() {
        return Err(anyhow!("failed to extract {}", tarball.display()));
    }
    Ok(())
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
            FeatureSource::Remote(_)
        ));
    }

    #[test]
    fn parse_remote_tarball() {
        assert!(matches!(
            FeatureSource::parse("https://example.com/feature.tgz"),
            FeatureSource::Remote(_)
        ));
    }

    #[test]
    fn parse_local() {
        assert!(matches!(
            FeatureSource::parse("./my-feature"),
            FeatureSource::Local(_)
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
        let df = feature_dockerfile("rust:latest", &plan);
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
        let df = feature_dockerfile("ubuntu:22.04", &plan);
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
