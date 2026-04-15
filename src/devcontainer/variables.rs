use std::path::Path;

enum DevcontainerVariable {
    LocalWorkspaceFolder,
    LocalWorkspaceFolderBasename,
    ContainerWorkspaceFolder,
    ContainerWorkspaceFolderBasename,
}

impl DevcontainerVariable {
    fn pattern(&self) -> &str {
        match self {
            Self::LocalWorkspaceFolder => "${localWorkspaceFolder}",
            Self::LocalWorkspaceFolderBasename => "${localWorkspaceFolderBasename}",
            Self::ContainerWorkspaceFolder => "${containerWorkspaceFolder}",
            Self::ContainerWorkspaceFolderBasename => "${containerWorkspaceFolderBasename}",
        }
    }

    fn resolve(&self, local_folder: &Path, container_workspace_folder: &str) -> String {
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
        }
    }
}

pub fn expand_variables(
    value: &str,
    local_folder: &Path,
    container_workspace_folder: &str,
) -> String {
    use DevcontainerVariable::*;
    [
        LocalWorkspaceFolder,
        LocalWorkspaceFolderBasename,
        ContainerWorkspaceFolder,
        ContainerWorkspaceFolderBasename,
    ]
    .iter()
    .fold(value.to_string(), |acc, var| {
        acc.replace(
            var.pattern(),
            &var.resolve(local_folder, container_workspace_folder),
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use random_string::{CharacterType, generate_random_string};
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
        );
        assert_eq!(result, format!("target={}", name));
    }

    #[test]
    fn when_expand_variables_with_no_variables_then_returns_unchanged() {
        let name = random_name();
        let local_folder = format!("/home/user/{}", name);
        let container_folder = format!("/workspaces/{}", name);
        let value = "type=bind,source=/host,target=/container";
        let result = expand_variables(value, Path::new(&local_folder), &container_folder);
        assert_eq!(result, value);
    }

    #[test]
    fn when_expand_variables_with_root_path_as_local_workspace_folder_basename_then_returns_empty()
    {
        let result = expand_variables(
            "${localWorkspaceFolderBasename}",
            Path::new("/"),
            "/workspaces/project",
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
        );
        assert_eq!(
            result,
            format!(
                "type=bind,source={},target={}",
                local_folder, container_folder
            )
        );
    }
}
