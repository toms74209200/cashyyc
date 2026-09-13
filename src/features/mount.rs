use crate::devcontainer::jsonc::Value;

#[derive(Debug, PartialEq)]
pub struct FeatureMount {
    pub mount_type: String,
    pub source: Option<String>,
    pub target: String,
}

impl FeatureMount {
    pub fn from_value(value: &Value) -> Option<Self> {
        let mount_type = value.get("type")?.as_str()?.to_string();
        let source = match value.get("source") {
            None | Some(Value::Null) => None,
            Some(v) => Some(v.as_str()?.to_string()),
        };
        let target = value.get("target")?.as_str()?.to_string();
        Some(FeatureMount {
            mount_type,
            source,
            target,
        })
    }

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
    use crate::devcontainer::jsonc;

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

    #[test]
    fn when_from_value_with_all_fields_then_parses_them() {
        let value = jsonc::parse(r#"{"type":"bind","source":"/a","target":"/b"}"#)
            .unwrap()
            .value();
        assert_eq!(
            FeatureMount::from_value(&value),
            Some(FeatureMount {
                mount_type: "bind".to_string(),
                source: Some("/a".to_string()),
                target: "/b".to_string(),
            })
        );
    }

    #[test]
    fn when_from_value_without_source_then_source_is_none() {
        let value = jsonc::parse(r#"{"type":"volume","target":"/b"}"#)
            .unwrap()
            .value();
        assert_eq!(
            FeatureMount::from_value(&value).and_then(|m| m.source),
            None
        );
    }

    #[test]
    fn when_from_value_without_type_then_returns_none() {
        let value = jsonc::parse(r#"{"target":"/b"}"#).unwrap().value();
        assert_eq!(FeatureMount::from_value(&value), None);
    }

    #[test]
    fn when_from_value_without_target_then_returns_none() {
        let value = jsonc::parse(r#"{"type":"bind"}"#).unwrap().value();
        assert_eq!(FeatureMount::from_value(&value), None);
    }
}
