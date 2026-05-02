use std::collections::HashMap;
use std::path::Path;

enum DevcontainerVariable {
    LocalWorkspaceFolder,
    LocalWorkspaceFolderBasename,
    ContainerWorkspaceFolder,
    ContainerWorkspaceFolderBasename,
    LocalEnv(String, Option<String>),
    ContainerEnv(String, Option<String>),
}

impl DevcontainerVariable {
    fn from_token(content: &str) -> Option<Self> {
        match content {
            "localWorkspaceFolder" => Some(Self::LocalWorkspaceFolder),
            "localWorkspaceFolderBasename" => Some(Self::LocalWorkspaceFolderBasename),
            "containerWorkspaceFolder" => Some(Self::ContainerWorkspaceFolder),
            "containerWorkspaceFolderBasename" => Some(Self::ContainerWorkspaceFolderBasename),
            _ => {
                let scope_end = content.find(':')?;
                let scope = &content[..scope_end];
                let rest = &content[scope_end + 1..];
                let (name, default) = match rest.split_once(':') {
                    Some((n, d)) => (n.to_string(), Some(d.to_string())),
                    None => (rest.to_string(), None),
                };
                match scope {
                    "localEnv" => Some(Self::LocalEnv(name, default)),
                    "containerEnv" => Some(Self::ContainerEnv(name, default)),
                    _ => None,
                }
            }
        }
    }

    fn resolve(
        &self,
        local_folder: &Path,
        container_workspace_folder: &str,
        container_env: &HashMap<String, String>,
    ) -> String {
        match self {
            Self::LocalWorkspaceFolder => local_folder.display().to_string(),
            Self::LocalWorkspaceFolderBasename => match local_folder.components().next_back() {
                Some(std::path::Component::Normal(name)) => name.to_string_lossy().to_string(),
                _ => String::new(),
            },
            Self::ContainerWorkspaceFolder => container_workspace_folder.to_string(),
            Self::ContainerWorkspaceFolderBasename => {
                match Path::new(container_workspace_folder)
                    .components()
                    .next_back()
                {
                    Some(std::path::Component::Normal(name)) => name.to_string_lossy().to_string(),
                    _ => String::new(),
                }
            }
            Self::LocalEnv(name, default) => {
                std::env::var(name).unwrap_or_else(|_| default.clone().unwrap_or_default())
            }
            Self::ContainerEnv(name, default) => container_env
                .get(name)
                .cloned()
                .unwrap_or_else(|| default.clone().unwrap_or_default()),
        }
    }
}

pub fn expand_variables(
    value: &str,
    local_folder: &Path,
    container_workspace_folder: &str,
    container_env: &HashMap<String, String>,
) -> String {
    let Some((first, rest)) = value.split_once("${") else {
        return value.to_string();
    };
    let mut result = first.to_string();
    for part in rest.split("${") {
        match part.split_once('}') {
            None => {
                result.push_str("${");
                result.push_str(part);
            }
            Some((content, tail)) => {
                let resolved = match DevcontainerVariable::from_token(content) {
                    Some(var) => {
                        var.resolve(local_folder, container_workspace_folder, container_env)
                    }
                    None => format!("${{{}}}", content),
                };
                result.push_str(&resolved);
                result.push_str(tail);
            }
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use random_string::{CharacterType, generate_random_string};
    use std::collections::HashMap;
    use std::fs::File;

    fn urandom() -> File {
        File::open("/dev/urandom").unwrap()
    }

    fn random_name() -> String {
        generate_random_string(8, &[CharacterType::Lowercase], "", &mut urandom())
    }

    #[test]
    fn when_expand_variables_with_local_workspace_folder_then_replaces_with_local_folder() {
        let name = random_name();
        let local_folder = format!("/home/user/{}", name);
        let container_folder = format!("/workspaces/{}", name);
        let result = expand_variables(
            "${localWorkspaceFolder}/data",
            Path::new(&local_folder),
            &container_folder,
            &HashMap::new(),
        );
        assert_eq!(result, format!("{}/data", local_folder));
    }

    #[test]
    fn when_expand_variables_with_local_workspace_folder_basename_then_replaces_with_basename() {
        let name = random_name();
        let local_folder = format!("/home/user/{}", name);
        let container_folder = format!("/workspaces/{}", name);
        let result = expand_variables(
            "source=${localWorkspaceFolderBasename}",
            Path::new(&local_folder),
            &container_folder,
            &HashMap::new(),
        );
        assert_eq!(result, format!("source={}", name));
    }

    #[test]
    fn when_expand_variables_with_container_workspace_folder_then_replaces_with_container_path() {
        let name = random_name();
        let local_folder = format!("/home/user/{}", name);
        let container_folder = format!("/workspaces/{}", name);
        let result = expand_variables(
            "${containerWorkspaceFolder}/build",
            Path::new(&local_folder),
            &container_folder,
            &HashMap::new(),
        );
        assert_eq!(result, format!("{}/build", container_folder));
    }

    #[test]
    fn when_expand_variables_with_container_workspace_folder_basename_then_replaces_with_basename()
    {
        let name = random_name();
        let local_folder = format!("/home/user/{}", name);
        let container_folder = format!("/workspaces/{}", name);
        let result = expand_variables(
            "target=${containerWorkspaceFolderBasename}",
            Path::new(&local_folder),
            &container_folder,
            &HashMap::new(),
        );
        assert_eq!(result, format!("target={}", name));
    }

    #[test]
    fn when_expand_variables_with_local_workspace_folder_basename_with_trailing_slash_then_replaces_with_basename()
     {
        let name = random_name();
        let local_folder = format!("/home/user/{}/", name);
        let container_folder = format!("/workspaces/{}", name);
        let result = expand_variables(
            "source=${localWorkspaceFolderBasename}",
            Path::new(&local_folder),
            &container_folder,
            &HashMap::new(),
        );
        assert_eq!(result, format!("source={}", name));
    }

    #[test]
    fn when_expand_variables_with_container_workspace_folder_basename_with_trailing_slash_then_replaces_with_basename()
     {
        let name = random_name();
        let local_folder = format!("/home/user/{}", name);
        let container_folder = format!("/workspaces/{}/", name);
        let result = expand_variables(
            "target=${containerWorkspaceFolderBasename}",
            Path::new(&local_folder),
            &container_folder,
            &HashMap::new(),
        );
        assert_eq!(result, format!("target={}", name));
    }

    #[test]
    fn when_expand_variables_with_no_variables_then_returns_unchanged() {
        let name = random_name();
        let local_folder = format!("/home/user/{}", name);
        let container_folder = format!("/workspaces/{}", name);
        let value = "type=bind,source=/host,target=/container";
        let result = expand_variables(
            value,
            Path::new(&local_folder),
            &container_folder,
            &HashMap::new(),
        );
        assert_eq!(result, value);
    }

    #[test]
    fn when_expand_variables_with_root_path_as_local_workspace_folder_basename_then_returns_empty()
    {
        let result = expand_variables(
            "${localWorkspaceFolderBasename}",
            Path::new("/"),
            "/workspaces/project",
            &HashMap::new(),
        );
        assert_eq!(result, "");
    }

    #[test]
    fn when_expand_variables_with_root_path_as_container_workspace_folder_basename_then_returns_empty()
     {
        let name = random_name();
        let local_folder = format!("/home/user/{}", name);
        let result = expand_variables(
            "${containerWorkspaceFolderBasename}",
            Path::new(&local_folder),
            "/",
            &HashMap::new(),
        );
        assert_eq!(result, "");
    }

    #[test]
    fn when_expand_variables_with_multiple_variables_then_replaces_all() {
        let name = random_name();
        let local_folder = format!("/home/user/{}", name);
        let container_folder = format!("/workspaces/{}", name);
        let result = expand_variables(
            "type=bind,source=${localWorkspaceFolder},target=${containerWorkspaceFolder}",
            Path::new(&local_folder),
            &container_folder,
            &HashMap::new(),
        );
        assert_eq!(
            result,
            format!(
                "type=bind,source={},target={}",
                local_folder, container_folder
            )
        );
    }

    #[test]
    fn when_expand_variables_with_local_env_then_expands_to_env_value() {
        let expected = std::env::var("PATH").unwrap_or_default();
        let result = expand_variables(
            "${localEnv:PATH}",
            Path::new("/home/user"),
            "/workspaces/x",
            &HashMap::new(),
        );
        assert_eq!(result, expected);
    }

    #[test]
    fn when_expand_variables_with_missing_local_env_then_expands_to_empty() {
        let result = expand_variables(
            "prefix_${localEnv:__CYYC_NO_SUCH_VAR__}_suffix",
            Path::new("/home/user"),
            "/workspaces/x",
            &HashMap::new(),
        );
        assert_eq!(result, "prefix__suffix");
    }

    #[test]
    fn when_expand_variables_with_multiple_local_env_then_expands_all() {
        let path = std::env::var("PATH").unwrap_or_default();
        let result = expand_variables(
            "${localEnv:PATH}:${localEnv:__CYYC_NO_SUCH_VAR__}",
            Path::new("/home/user"),
            "/workspaces/x",
            &HashMap::new(),
        );
        assert_eq!(result, format!("{}:", path));
    }

    #[test]
    fn when_expand_variables_with_unclosed_local_env_brace_then_treats_literally() {
        let result = expand_variables(
            "${localEnv:PATH",
            Path::new("/home/user"),
            "/workspaces/x",
            &HashMap::new(),
        );
        assert_eq!(result, "${localEnv:PATH");
    }

    #[test]
    fn when_expand_variables_with_local_env_with_default_and_env_set_then_uses_env_value() {
        let expected = std::env::var("PATH").unwrap_or_default();
        let result = expand_variables(
            "${localEnv:PATH:fallback}",
            Path::new("/home/user"),
            "/workspaces/x",
            &HashMap::new(),
        );
        assert_eq!(result, expected);
    }

    #[test]
    fn when_expand_variables_with_local_env_with_default_and_env_missing_then_uses_default() {
        let result = expand_variables(
            "${localEnv:__CYYC_NO_SUCH_VAR__:mydefault}",
            Path::new("/home/user"),
            "/workspaces/x",
            &HashMap::new(),
        );
        assert_eq!(result, "mydefault");
    }

    #[test]
    fn when_expand_variables_with_container_env_then_expands_to_container_env_value() {
        let mut env = HashMap::new();
        env.insert("HOME".to_string(), "/root".to_string());
        let result = expand_variables(
            "${containerEnv:HOME}",
            Path::new("/home/user"),
            "/workspaces/x",
            &env,
        );
        assert_eq!(result, "/root");
    }

    #[test]
    fn when_expand_variables_with_missing_container_env_then_expands_to_empty() {
        let result = expand_variables(
            "prefix_${containerEnv:__NO_SUCH_VAR__}_suffix",
            Path::new("/home/user"),
            "/workspaces/x",
            &HashMap::new(),
        );
        assert_eq!(result, "prefix__suffix");
    }

    #[test]
    fn when_expand_variables_with_container_env_with_default_and_env_missing_then_uses_default() {
        let result = expand_variables(
            "${containerEnv:__NO_SUCH_VAR__:mydefault}",
            Path::new("/home/user"),
            "/workspaces/x",
            &HashMap::new(),
        );
        assert_eq!(result, "mydefault");
    }

    #[test]
    fn when_expand_variables_with_container_env_with_default_and_env_present_then_uses_container_value()
     {
        let mut env = HashMap::new();
        env.insert("MYVAR".to_string(), "containervalue".to_string());
        let result = expand_variables(
            "${containerEnv:MYVAR:fallback}",
            Path::new("/home/user"),
            "/workspaces/x",
            &env,
        );
        assert_eq!(result, "containervalue");
    }

    #[test]
    fn when_expand_variables_with_multiple_container_env_then_expands_all() {
        let mut env = HashMap::new();
        env.insert("FOO".to_string(), "foo_val".to_string());
        let result = expand_variables(
            "${containerEnv:FOO}:${containerEnv:__NO_SUCH_VAR__}",
            Path::new("/home/user"),
            "/workspaces/x",
            &env,
        );
        assert_eq!(result, "foo_val:");
    }

    #[test]
    fn when_expand_variables_with_unclosed_container_env_brace_then_treats_literally() {
        let result = expand_variables(
            "${containerEnv:HOME",
            Path::new("/home/user"),
            "/workspaces/x",
            &HashMap::new(),
        );
        assert_eq!(result, "${containerEnv:HOME");
    }
}
