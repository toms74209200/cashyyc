use super::jsonc::Value;
use super::variables::expand_variables;
use crate::err;
use crate::error::Result;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Default, PartialEq)]
pub struct RemoteEnv(pub HashMap<String, Value>);

impl RemoteEnv {
    pub fn exec_args(
        &self,
        local_folder: &Path,
        container_workspace_folder: &str,
        container_env: &HashMap<String, String>,
        local_env: &HashMap<String, String>,
    ) -> Result<Vec<String>> {
        fn js_number(n: f64) -> String {
            if n == 0.0 {
                return "0".to_string();
            }
            if n.is_infinite() {
                return if n > 0.0 { "Infinity" } else { "-Infinity" }.to_string();
            }
            if n < 0.0 {
                return format!("-{}", js_number(-n));
            }
            let scientific = format!("{n:e}");
            let (mantissa, exponent) = scientific
                .split_once('e')
                .unwrap_or((scientific.as_str(), "0"));
            let digits: String = mantissa.chars().filter(|c| *c != '.').collect();
            let k = digits.len() as i32;
            let point = exponent.parse::<i32>().unwrap_or(0) + 1;
            if k <= point && point <= 21 {
                format!("{digits}{}", "0".repeat((point - k) as usize))
            } else if 0 < point && point <= 21 {
                let (integer, fraction) = digits.split_at(point as usize);
                format!("{integer}.{fraction}")
            } else if -6 < point && point <= 0 {
                format!("0.{}{digits}", "0".repeat(-point as usize))
            } else {
                let (head, tail) = digits.split_at(1);
                let fraction = if tail.is_empty() {
                    String::new()
                } else {
                    format!(".{tail}")
                };
                let sign = if point > 0 { '+' } else { '-' };
                format!("{head}{fraction}e{sign}{}", (point - 1).abs())
            }
        }

        fn js_string(value: &Value, expand: &impl Fn(&str) -> String) -> Option<String> {
            match value {
                Value::Null => Some("null".to_string()),
                Value::Bool(b) => Some(b.to_string()),
                Value::Number(n) => Some(js_number(*n)),
                Value::String(s) => Some(expand(s)),
                Value::Array(items) => items
                    .iter()
                    .map(|item| match item {
                        Value::Null => Some(String::new()),
                        item => js_string(item, expand),
                    })
                    .collect::<Option<Vec<_>>>()
                    .map(|items| items.join(",")),
                Value::Object(_) => None,
            }
        }

        let expand = |s: &str| {
            expand_variables(
                s,
                local_folder,
                container_workspace_folder,
                container_env,
                local_env,
            )
        };
        let mut members: Vec<(&String, &Value)> = self.0.iter().collect();
        members.sort_by_key(|(key, _)| key.as_str());
        members
            .into_iter()
            .map(|(key, value)| {
                let value = js_string(value, &expand)
                    .ok_or_else(|| err!("remoteEnv `{key}` cannot be converted to a string"))?;
                Ok(["--env".to_string(), format!("{key}={value}")])
            })
            .collect::<Result<Vec<_>>>()
            .map(|args| args.concat())
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

    fn exec_args_of(members: &[(&str, &str)]) -> Result<Vec<String>> {
        RemoteEnv(
            members
                .iter()
                .map(|(key, json)| (key.to_string(), jsonc::parse(json).unwrap().value()))
                .collect(),
        )
        .exec_args(
            Path::new("/home/user/project"),
            "/workspaces/project",
            &HashMap::new(),
            &HashMap::new(),
        )
    }

    #[test]
    fn when_exec_args_then_returns_env_flags_in_key_order() {
        let args = exec_args_of(&[("B", r#""b""#), ("A", r#""a""#)]).unwrap();

        assert_eq!(args, ["--env", "A=a", "--env", "B=b"]);
    }

    #[test]
    fn when_exec_args_with_empty_remote_env_then_returns_no_args() {
        assert_eq!(exec_args_of(&[]).unwrap(), Vec::<String>::new());
    }

    #[test]
    fn when_exec_args_with_scalar_values_then_converts_them_as_devcontainer_cli_does() {
        for (json, expected) in [
            (r#""x""#, "x"),
            (r#""""#, ""),
            ("true", "true"),
            ("false", "false"),
            ("null", "null"),
            ("1", "1"),
            ("-1", "-1"),
            ("1.5", "1.5"),
            ("-0.0", "0"),
            ("0.000001", "0.000001"),
            ("1e-7", "1e-7"),
            ("1.5e-7", "1.5e-7"),
            ("123456789012345680000", "123456789012345680000"),
            ("1e21", "1e+21"),
            ("1.5e300", "1.5e+300"),
        ] {
            assert_eq!(
                exec_args_of(&[("R", json)]).unwrap(),
                ["--env".to_string(), format!("R={expected}")],
                "value: {json}"
            );
        }
    }

    #[test]
    fn when_exec_args_with_array_values_then_joins_elements_with_commas() {
        for (json, expected) in [
            ("[]", ""),
            (r#"[1,"a",null]"#, "1,a,"),
            ("[[1,2],3,true]", "1,2,3,true"),
        ] {
            assert_eq!(
                exec_args_of(&[("R", json)]).unwrap(),
                ["--env".to_string(), format!("R={expected}")],
                "value: {json}"
            );
        }
    }

    #[test]
    fn when_exec_args_with_object_value_then_returns_error() {
        for json in [r#"{"k":"v"}"#, r#"[{"k":"v"}]"#] {
            assert!(exec_args_of(&[("R", json)]).is_err(), "value: {json}");
        }
    }

    #[test]
    fn when_exec_args_with_variables_then_expands_strings_and_array_elements() {
        let (container_value, local_value) = (random_word(), random_word());
        let remote_env = RemoteEnv(HashMap::from([
            (
                "C".to_string(),
                Value::String("${containerEnv:CYYC_C}".to_string()),
            ),
            (
                "L".to_string(),
                Value::String("${localEnv:CYYC_L}".to_string()),
            ),
            (
                "W".to_string(),
                jsonc::parse(r#"["${containerWorkspaceFolder}","${containerEnv:CYYC_MISSING}"]"#)
                    .unwrap()
                    .value(),
            ),
        ]));

        let args = remote_env
            .exec_args(
                Path::new("/home/user/project"),
                "/workspaces/project",
                &HashMap::from([("CYYC_C".to_string(), container_value.clone())]),
                &HashMap::from([("CYYC_L".to_string(), local_value.clone())]),
            )
            .unwrap();

        assert_eq!(
            args,
            [
                "--env".to_string(),
                format!("C={container_value}"),
                "--env".to_string(),
                format!("L={local_value}"),
                "--env".to_string(),
                "W=/workspaces/project,".to_string(),
            ]
        );
    }
}
