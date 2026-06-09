pub enum FeatureInstallUsers {
    BothNamed { container: String, remote: String },
    ContainerOnly(String),
    RemoteOnly(String),
    NeitherNamed,
}

impl FeatureInstallUsers {
    pub fn new(container_user: Option<&str>, remote_user: Option<&str>) -> Self {
        match (container_user, remote_user) {
            (Some(c), Some(r)) => Self::BothNamed {
                container: c.to_string(),
                remote: r.to_string(),
            },
            (Some(c), None) => Self::ContainerOnly(c.to_string()),
            (None, Some(r)) => Self::RemoteOnly(r.to_string()),
            (None, None) => Self::NeitherNamed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn when_new_with_container_user_and_remote_user_then_both_named() {
        assert!(matches!(
            FeatureInstallUsers::new(Some("root"), Some("vscode")),
            FeatureInstallUsers::BothNamed { container, remote } if container == "root" && remote == "vscode"
        ));
    }

    #[test]
    fn when_new_with_container_user_and_no_remote_user_then_container_only() {
        assert!(matches!(
            FeatureInstallUsers::new(Some("root"), None),
            FeatureInstallUsers::ContainerOnly(c) if c == "root"
        ));
    }

    #[test]
    fn when_new_with_no_container_user_and_remote_user_then_remote_only() {
        assert!(matches!(
            FeatureInstallUsers::new(None, Some("vscode")),
            FeatureInstallUsers::RemoteOnly(r) if r == "vscode"
        ));
    }

    #[test]
    fn when_new_with_no_container_user_and_no_remote_user_then_neither_named() {
        assert!(matches!(
            FeatureInstallUsers::new(None, None),
            FeatureInstallUsers::NeitherNamed
        ));
    }
}
