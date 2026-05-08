use serde_json::Value;

#[derive(Debug, PartialEq)]
pub enum LifecycleCmd {
    Shell(String),
    Exec(Vec<String>),
    Parallel(Vec<LifecycleCmd>),
}

impl TryFrom<&Value> for LifecycleCmd {
    type Error = ();

    fn try_from(value: &Value) -> Result<Self, Self::Error> {
        match value {
            Value::String(s) => Ok(LifecycleCmd::Shell(s.clone())),
            Value::Array(arr) => Ok(LifecycleCmd::Exec(
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect(),
            )),
            Value::Object(map) => Ok(LifecycleCmd::Parallel(
                map.values()
                    .filter_map(|v| Self::try_from(v).ok())
                    .collect(),
            )),
            _ => Err(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn when_string_value_then_becomes_shell() {
        assert_eq!(
            LifecycleCmd::try_from(&json!("npm install")).unwrap(),
            LifecycleCmd::Shell("npm install".to_string())
        );
    }

    #[test]
    fn when_array_value_then_becomes_exec() {
        assert_eq!(
            LifecycleCmd::try_from(&json!(["npm", "install"])).unwrap(),
            LifecycleCmd::Exec(vec!["npm".to_string(), "install".to_string()])
        );
    }

    #[test]
    fn when_object_value_then_becomes_parallel_of_shell() {
        assert_eq!(
            LifecycleCmd::try_from(&json!({"install": "npm install", "build": "npm run build"}))
                .unwrap(),
            LifecycleCmd::Parallel(vec![
                LifecycleCmd::Shell("npm run build".to_string()),
                LifecycleCmd::Shell("npm install".to_string()),
            ])
        );
    }

    #[test]
    fn when_object_with_array_value_then_becomes_parallel_of_exec() {
        assert_eq!(
            LifecycleCmd::try_from(&json!({"run": ["npm", "install"]})).unwrap(),
            LifecycleCmd::Parallel(vec![LifecycleCmd::Exec(vec![
                "npm".to_string(),
                "install".to_string(),
            ])])
        );
    }

    #[test]
    fn when_array_with_non_string_elements_then_non_strings_are_skipped() {
        assert_eq!(
            LifecycleCmd::try_from(&json!(["npm", 1, "install"])).unwrap(),
            LifecycleCmd::Exec(vec!["npm".to_string(), "install".to_string()])
        );
    }

    #[test]
    fn when_object_with_invalid_sub_value_then_it_is_excluded_from_parallel() {
        assert_eq!(
            LifecycleCmd::try_from(&json!({"valid": "echo hi", "invalid": null})).unwrap(),
            LifecycleCmd::Parallel(vec![LifecycleCmd::Shell("echo hi".to_string())])
        );
    }

    #[test]
    fn when_null_value_then_returns_err() {
        assert!(LifecycleCmd::try_from(&json!(null)).is_err());
    }
}
