pub enum PathDomain {
    Unix(std::path::PathBuf),
    Wsl(String),
}

impl PathDomain {
    pub fn filter_string(&self) -> String {
        match self {
            PathDomain::Unix(cwd) => {
                format!("label=devcontainer.local_folder={}", cwd.display())
            }
            PathDomain::Wsl(wsl_path) => {
                format!("label=devcontainer.local_folder={}", wsl_path)
            }
        }
    }
}

pub fn parse_remote_user_from_metadata(metadata: &str) -> Option<String> {
    let arr = serde_json::from_str::<serde_json::Value>(metadata).ok()?;
    arr.as_array()?.iter().find_map(|obj| {
        obj.get("remoteUser")
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
    })
}

pub fn parse_container_id(output: &str) -> Option<String> {
    output
        .lines()
        .find(|line| !line.trim().is_empty())
        .map(|line| line.trim().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use random_string::{CharacterType, generate_random_string};
    use std::fs::File;

    fn urandom() -> File {
        File::open("/dev/urandom").unwrap()
    }

    #[test]
    fn when_linux_domain_filter_string_then_returns_linux_path_label() {
        let domain = PathDomain::Unix(std::path::PathBuf::from("/home/user/projects/cashyyc"));
        assert_eq!(
            domain.filter_string(),
            "label=devcontainer.local_folder=/home/user/projects/cashyyc"
        );
    }

    #[test]
    fn when_wsl_domain_filter_string_then_returns_wsl_path_label() {
        let domain = PathDomain::Wsl(
            "\\\\wsl.localhost\\Ubuntu-20.04\\home\\user\\projects\\cashyyc".to_string(),
        );
        assert_eq!(
            domain.filter_string(),
            "label=devcontainer.local_folder=\\\\wsl.localhost\\Ubuntu-20.04\\home\\user\\projects\\cashyyc"
        );
    }

    #[test]
    fn when_parse_remote_user_from_metadata_with_remote_user_then_returns_some() {
        let metadata = r#"[{"id":"feature:1"},{"remoteUser":"vscode"},{"id":"feature:2"}]"#;
        assert_eq!(
            parse_remote_user_from_metadata(metadata),
            Some("vscode".to_string())
        );
    }

    #[test]
    fn when_parse_remote_user_from_metadata_without_remote_user_then_returns_none() {
        let metadata = r#"[{"id":"feature:1"},{"id":"feature:2"}]"#;
        assert_eq!(parse_remote_user_from_metadata(metadata), None);
    }

    #[test]
    fn when_parse_remote_user_from_metadata_with_empty_string_then_returns_none() {
        assert_eq!(parse_remote_user_from_metadata(""), None);
    }

    #[test]
    fn when_parse_remote_user_from_metadata_with_empty_remote_user_then_returns_none() {
        let metadata = r#"[{"id":"feature:1"},{"remoteUser":""},{"id":"feature:2"}]"#;
        assert_eq!(parse_remote_user_from_metadata(metadata), None);
    }

    #[test]
    fn when_parse_container_id_with_container_id_then_returns_some() {
        let id = generate_random_string(
            12,
            &[CharacterType::Lowercase, CharacterType::Numeric],
            "",
            &mut urandom(),
        );
        let output = format!("{}\n", id);
        assert_eq!(parse_container_id(&output), Some(id));
    }

    #[test]
    fn when_parse_container_id_with_empty_output_then_returns_none() {
        assert_eq!(parse_container_id(""), None);
    }

    #[test]
    fn when_parse_container_id_with_newline_only_then_returns_none() {
        assert_eq!(parse_container_id("\n"), None);
    }
}
