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

pub struct ImageConfig {
    pub entrypoint: Vec<String>,
    pub cmd: Vec<String>,
}

impl ImageConfig {
    pub fn parse(json: &str) -> Self {
        let v: serde_json::Value = serde_json::from_str(json.trim()).unwrap_or_default();
        let strings = |key: &str| -> Vec<String> {
            v[key]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|e| e.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default()
        };
        Self {
            entrypoint: strings("Entrypoint"),
            cmd: strings("Cmd"),
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

pub fn parse_container_ids(output: &str) -> Vec<String> {
    output
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| line.trim().to_string())
        .collect()
}

pub struct Container {
    pub id: String,
    pub remote_user: Option<String>,
}

pub fn find_container(
    inspect_json: &str,
    config_path: &std::path::Path,
    cwd: &std::path::Path,
) -> Option<Container> {
    let arr = serde_json::from_str::<serde_json::Value>(inspect_json).ok()?;
    let arr = arr.as_array()?;
    let cwd_basename = cwd.file_name()?.to_string_lossy().to_string();
    let rel = config_path.strip_prefix(cwd).ok()?;
    let config_suffix = format!("/{}", rel.display());
    for c in arr {
        let id = match c.get("Id").and_then(|v| v.as_str()) {
            Some(id) => id.to_string(),
            None => continue,
        };
        let labels = match c.get("Config").and_then(|c| c.get("Labels")) {
            Some(l) => l,
            None => continue,
        };
        let local_folder = labels
            .get("devcontainer.local_folder")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let config_file = labels
            .get("devcontainer.config_file")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let norm_local = local_folder.replace('\\', "/");
        let label_basename = norm_local
            .trim_end_matches('/')
            .rsplit('/')
            .next()
            .unwrap_or("");
        if label_basename != cwd_basename {
            continue;
        }
        let norm_config = config_file.replace('\\', "/");
        if !norm_config.ends_with(&config_suffix) {
            continue;
        }
        let metadata = labels
            .get("devcontainer.metadata")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let config_user = c
            .get("Config")
            .and_then(|c| c.get("User"))
            .and_then(|v| v.as_str())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());
        let remote_user = parse_remote_user_from_metadata(metadata).or(config_user);
        return Some(Container { id, remote_user });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use random_string::{CharacterType, generate_random_string};
    use std::fs::File;
    use std::path::Path;

    fn urandom() -> File {
        File::open("/dev/urandom").unwrap()
    }

    fn random_id() -> String {
        generate_random_string(
            12,
            &[CharacterType::Lowercase, CharacterType::Numeric],
            "",
            &mut urandom(),
        )
    }

    fn make_inspect_json(
        id: &str,
        local_folder: &str,
        config_file: &str,
        metadata: &str,
        user: &str,
    ) -> String {
        format!(
            r#"[{{"Id":"{id}","Config":{{"User":"{user}","Labels":{{"devcontainer.local_folder":"{local_folder}","devcontainer.config_file":"{config_file}","devcontainer.metadata":"{metadata}"}}}}}}]"#,
        )
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
        let id = random_id();
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
    fn when_parse_container_ids_with_multiple_ids_then_returns_all() {
        let id1 = random_id();
        let id2 = random_id();
        let output = format!("{}\n{}\n", id1, id2);
        assert_eq!(parse_container_ids(&output), vec![id1, id2]);
    }

    #[test]
    fn when_parse_container_ids_with_empty_string_then_returns_empty() {
        assert_eq!(parse_container_ids(""), Vec::<String>::new());
    }

    #[test]
    fn when_parse_container_ids_with_single_id_then_returns_vec_with_one() {
        let id = random_id();
        let output = format!("{}\n", id);
        assert_eq!(parse_container_ids(&output), vec![id]);
    }

    #[test]
    fn when_find_container_with_matching_linux_labels_then_returns_container() {
        let id = random_id();
        let json = make_inspect_json(
            &id,
            "/host/project",
            "/host/project/.devcontainer/devcontainer.json",
            "[]",
            "",
        );
        let result = find_container(
            &json,
            Path::new("/cwd/project/.devcontainer/devcontainer.json"),
            Path::new("/cwd/project"),
        );
        assert_eq!(result.map(|c| c.id), Some(id));
    }

    #[test]
    fn when_find_container_with_matching_windows_labels_then_returns_container() {
        let id = random_id();
        let json = make_inspect_json(
            &id,
            "C:\\\\Users\\\\user\\\\project",
            "C:\\\\Users\\\\user\\\\project\\\\.devcontainer\\\\devcontainer.json",
            "[]",
            "",
        );
        let result = find_container(
            &json,
            Path::new("/cwd/project/.devcontainer/devcontainer.json"),
            Path::new("/cwd/project"),
        );
        assert!(result.is_some());
    }

    #[test]
    fn when_find_container_with_matching_wsl_local_folder_then_returns_container() {
        let id = random_id();
        let json = make_inspect_json(
            &id,
            "\\\\\\\\wsl.localhost\\\\Ubuntu\\\\home\\\\user\\\\project",
            "/home/user/project/.devcontainer/devcontainer.json",
            "[]",
            "",
        );
        let result = find_container(
            &json,
            Path::new("/cwd/project/.devcontainer/devcontainer.json"),
            Path::new("/cwd/project"),
        );
        assert!(result.is_some());
    }

    #[test]
    fn when_find_container_with_different_basename_then_returns_none() {
        let id = random_id();
        let json = make_inspect_json(
            &id,
            "/host/other-project",
            "/host/other-project/.devcontainer/devcontainer.json",
            "[]",
            "",
        );
        let result = find_container(
            &json,
            Path::new("/cwd/project/.devcontainer/devcontainer.json"),
            Path::new("/cwd/project"),
        );
        assert!(result.is_none());
    }

    #[test]
    fn when_find_container_with_different_config_file_then_returns_none() {
        let id = random_id();
        let json = make_inspect_json(
            &id,
            "/host/project",
            "/host/project/.devcontainer/other/devcontainer.json",
            "[]",
            "",
        );
        let result = find_container(
            &json,
            Path::new("/cwd/project/.devcontainer/config1/devcontainer.json"),
            Path::new("/cwd/project"),
        );
        assert!(result.is_none());
    }

    #[test]
    fn when_find_container_with_remote_user_in_metadata_then_returns_it() {
        let id = random_id();
        let json = make_inspect_json(
            &id,
            "/host/project",
            "/host/project/.devcontainer/devcontainer.json",
            r#"[{\"remoteUser\":\"vscode\"}]"#,
            "",
        );
        let result = find_container(
            &json,
            Path::new("/cwd/project/.devcontainer/devcontainer.json"),
            Path::new("/cwd/project"),
        );
        assert_eq!(
            result.and_then(|c| c.remote_user),
            Some("vscode".to_string())
        );
    }

    #[test]
    fn when_find_container_with_remote_user_in_config_user_then_returns_it() {
        let id = random_id();
        let json = make_inspect_json(
            &id,
            "/host/project",
            "/host/project/.devcontainer/devcontainer.json",
            "[]",
            "node",
        );
        let result = find_container(
            &json,
            Path::new("/cwd/project/.devcontainer/devcontainer.json"),
            Path::new("/cwd/project"),
        );
        assert_eq!(result.and_then(|c| c.remote_user), Some("node".to_string()));
    }

    #[test]
    fn when_find_container_with_empty_json_then_returns_none() {
        assert!(
            find_container(
                "",
                Path::new("/cwd/project/.devcontainer/devcontainer.json"),
                Path::new("/cwd/project"),
            )
            .is_none()
        );
    }

    #[test]
    fn when_find_container_with_multiple_candidates_then_returns_first_match() {
        let id1 = random_id();
        let id2 = random_id();
        let json = format!(
            r#"[{{"Id":"{id1}","Config":{{"User":"","Labels":{{"devcontainer.local_folder":"/host/other","devcontainer.config_file":"/host/other/.devcontainer/devcontainer.json","devcontainer.metadata":"[]"}}}}}},{{"Id":"{id2}","Config":{{"User":"","Labels":{{"devcontainer.local_folder":"/host/project","devcontainer.config_file":"/host/project/.devcontainer/devcontainer.json","devcontainer.metadata":"[]"}}}}}}]"#
        );
        let result = find_container(
            &json,
            Path::new("/cwd/project/.devcontainer/devcontainer.json"),
            Path::new("/cwd/project"),
        );
        assert_eq!(result.map(|c| c.id), Some(id2));
    }

    #[test]
    fn when_parse_image_config_json_with_array_then_returns_vec() {
        assert_eq!(
            parse_image_config_json(r#"["/entrypoint.sh","--flag"]"#),
            vec!["/entrypoint.sh".to_string(), "--flag".to_string()]
        );
    }

    #[test]
    fn when_image_config_parse_with_entrypoint_and_cmd_then_returns_both() {
        let json = r#"{"Entrypoint":["/entrypoint.sh"],"Cmd":["start"]}"#;
        let c = ImageConfig::parse(json);
        assert_eq!(c.entrypoint, vec!["/entrypoint.sh"]);
        assert_eq!(c.cmd, vec!["start"]);
    }

    #[test]
    fn when_image_config_parse_with_null_fields_then_returns_empty_vecs() {
        let json = r#"{"Entrypoint":null,"Cmd":null}"#;
        let c = ImageConfig::parse(json);
        assert!(c.entrypoint.is_empty());
        assert!(c.cmd.is_empty());
    }

    #[test]
    fn when_image_config_parse_with_invalid_json_then_returns_empty_vecs() {
        let c = ImageConfig::parse("");
        assert!(c.entrypoint.is_empty());
        assert!(c.cmd.is_empty());
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
