use serde::Deserialize;

#[derive(Deserialize, Debug, PartialEq)]
pub struct FeatureMount {
    #[serde(rename = "type")]
    pub mount_type: String,
    pub source: Option<String>,
    pub target: String,
}

impl FeatureMount {
    pub fn to_docker_arg(&self) -> String {
        let src = self
            .source
            .as_deref()
            .map(|s| format!(",src={s}"))
            .unwrap_or_default();
        format!("type={}{src},dst={}", self.mount_type, self.target)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn when_to_docker_arg_with_bind_mount_then_formats_type_src_dst() {
        let mount = FeatureMount {
            mount_type: "bind".to_string(),
            source: Some("/var/run/docker.sock".to_string()),
            target: "/var/run/docker-host.sock".to_string(),
        };
        assert_eq!(
            mount.to_docker_arg(),
            "type=bind,src=/var/run/docker.sock,dst=/var/run/docker-host.sock"
        );
    }

    #[test]
    fn when_to_docker_arg_without_source_then_omits_src() {
        let mount = FeatureMount {
            mount_type: "volume".to_string(),
            source: None,
            target: "/var/lib/docker".to_string(),
        };
        assert_eq!(mount.to_docker_arg(), "type=volume,dst=/var/lib/docker");
    }
}
