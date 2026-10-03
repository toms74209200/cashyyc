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
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devcontainer::jsonc;

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
