fn fnv1a(s: &str) -> u64 {
    const OFFSET: u64 = 14695981039346656037;
    const PRIME: u64 = 1099511628211;
    s.bytes()
        .fold(OFFSET, |h, b| (h ^ b as u64).wrapping_mul(PRIME))
}

pub fn image_tag(local_folder: &std::path::Path) -> String {
    let raw = local_folder
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    let filtered: String = raw
        .chars()
        .filter(|c| matches!(c, 'a'..='z' | '0'..='9' | '.' | '_' | '-'))
        .collect();
    let name = filtered.trim_start_matches(['.', '-']);
    let name = if name.is_empty() { "workspace" } else { name };
    let hash = fnv1a(&local_folder.display().to_string().to_lowercase());
    format!("vsc-{}-{:016x}", name, hash)
}

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

pub fn parse_image_config_json(json: &str) -> Vec<String> {
    serde_json::from_str::<serde_json::Value>(json.trim())
        .ok()
        .and_then(|v| {
            if v.is_null() {
                Some(vec![])
            } else {
                v.as_array().map(|arr| {
                    arr.iter()
                        .filter_map(|s| s.as_str().map(|s| s.to_string()))
                        .collect()
                })
            }
        })
        .unwrap_or_default()
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
    fn when_unix_domain_filter_string_then_returns_unix_path_label() {
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

    #[test]
    fn when_parse_image_config_json_with_array_then_returns_vec() {
        assert_eq!(
            parse_image_config_json(r#"["/entrypoint.sh","--flag"]"#),
            vec!["/entrypoint.sh".to_string(), "--flag".to_string()]
        );
    }

    #[test]
    fn when_parse_image_config_json_with_null_then_returns_empty_vec() {
        assert_eq!(parse_image_config_json("null"), Vec::<String>::new());
    }

    #[test]
    fn when_parse_image_config_json_with_empty_array_then_returns_empty_vec() {
        assert_eq!(parse_image_config_json("[]"), Vec::<String>::new());
    }

    #[test]
    fn when_parse_image_config_json_with_invalid_then_returns_empty_vec() {
        assert_eq!(parse_image_config_json(""), Vec::<String>::new());
    }

    #[test]
    fn when_image_tag_then_starts_with_vsc_and_folder_name() {
        let tag = image_tag(std::path::Path::new("/home/user/myproject"));
        assert!(tag.starts_with("vsc-myproject-"));
    }

    #[test]
    fn when_image_tag_with_same_path_then_same_result() {
        let path = std::path::Path::new("/home/user/myproject");
        assert_eq!(image_tag(path), image_tag(path));
    }

    #[test]
    fn when_image_tag_with_different_paths_then_different_results() {
        let a = image_tag(std::path::Path::new("/home/user/project-a"));
        let b = image_tag(std::path::Path::new("/home/user/project-b"));
        assert_ne!(a, b);
    }

    #[test]
    fn when_image_tag_with_uppercase_folder_name_then_lowercase_in_tag() {
        let tag = image_tag(std::path::Path::new("/home/user/MyProject"));
        assert!(tag.starts_with("vsc-myproject-"));
    }

    #[test]
    fn when_image_tag_with_special_chars_in_folder_name_then_removed() {
        let tag = image_tag(std::path::Path::new("/home/user/my@project"));
        assert!(tag.starts_with("vsc-myproject-"));
    }

    #[test]
    fn when_image_tag_with_spaces_in_folder_name_then_removed() {
        let tag = image_tag(std::path::Path::new("/home/user/my project"));
        assert!(tag.starts_with("vsc-myproject-"));
    }

    #[test]
    fn when_image_tag_with_leading_hyphen_in_folder_name_then_trimmed() {
        let tag = image_tag(std::path::Path::new("/home/user/-myproject"));
        assert!(tag.starts_with("vsc-myproject-"));
    }

    #[test]
    fn when_image_tag_with_leading_period_in_folder_name_then_trimmed() {
        let tag = image_tag(std::path::Path::new("/home/user/.myproject"));
        assert!(tag.starts_with("vsc-myproject-"));
    }

    #[test]
    fn when_image_tag_with_only_invalid_chars_in_folder_name_then_workspace() {
        let tag = image_tag(std::path::Path::new("/home/user/@@@"));
        assert!(tag.starts_with("vsc-workspace-"));
    }
}
