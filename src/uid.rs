use crate::devcontainer::config::CommonConfig;
use std::path::Path;

pub const UPDATE_UID_DOCKERFILE: &str = r#"ARG BASE_IMAGE
FROM $BASE_IMAGE

USER root

ARG REMOTE_USER
ARG NEW_UID
ARG NEW_GID
SHELL ["/bin/sh", "-c"]
RUN eval $(sed -n "s/${REMOTE_USER}:[^:]*:\([^:]*\):\([^:]*\):[^:]*:\([^:]*\).*/OLD_UID=\1;OLD_GID=\2;HOME_FOLDER=\3/p" /etc/passwd); \
	eval $(sed -n "s/\([^:]*\):[^:]*:${NEW_UID}:.*/EXISTING_USER=\1/p" /etc/passwd); \
	eval $(sed -n "s/\([^:]*\):[^:]*:${NEW_GID}:.*/EXISTING_GROUP=\1/p" /etc/group); \
	if [ -z "$OLD_UID" ]; then \
		echo "Remote user not found in /etc/passwd ($REMOTE_USER)."; \
	elif [ "$OLD_UID" = "$NEW_UID" -a "$OLD_GID" = "$NEW_GID" ]; then \
		echo "UIDs and GIDs are the same ($NEW_UID:$NEW_GID)."; \
	elif [ "$OLD_UID" != "$NEW_UID" -a -n "$EXISTING_USER" ]; then \
		echo "User with UID exists ($EXISTING_USER=$NEW_UID)."; \
	else \
		if [ "$OLD_GID" != "$NEW_GID" -a -n "$EXISTING_GROUP" ]; then \
			echo "Group with GID exists ($EXISTING_GROUP=$NEW_GID)."; \
			NEW_GID="$OLD_GID"; \
		fi; \
		echo "Updating UID:GID from $OLD_UID:$OLD_GID to $NEW_UID:$NEW_GID."; \
		sed -i -e "s/\(${REMOTE_USER}:[^:]*:\)[^:]*:[^:]*/\1${NEW_UID}:${NEW_GID}/" /etc/passwd; \
		if [ "$OLD_GID" != "$NEW_GID" ]; then \
			sed -i -e "s/\([^:]*:[^:]*:\)${OLD_GID}:/\1${NEW_GID}:/" /etc/group; \
		fi; \
		chown -R $NEW_UID:$NEW_GID $HOME_FOLDER; \
	fi;

ARG IMAGE_USER
USER $IMAGE_USER
"#;

pub enum UidContext<'a> {
    Single {
        base_image: &'a str,
        image_user: &'a str,
    },
    Compose {
        override_content: &'a str,
        service: &'a str,
        image: &'a str,
        image_user: &'a str,
    },
}

pub enum UidUpdate {
    Single {
        uid_tag: String,
        remote_user: String,
        new_uid: u32,
        new_gid: u32,
        image_user: String,
    },
    Compose {
        uid_tag: String,
        remote_user: String,
        new_uid: u32,
        new_gid: u32,
        image_user: String,
        override_content: String,
    },
}

impl UidUpdate {
    pub fn resolve(
        ctx: UidContext,
        common: &CommonConfig,
        host_uid: u32,
        host_gid: u32,
        cwd: &Path,
    ) -> Option<Self> {
        if common.update_remote_user_uid == Some(false) {
            return None;
        }

        let (base_image, image_user) = match &ctx {
            UidContext::Single {
                base_image,
                image_user,
            } => (*base_image, *image_user),
            UidContext::Compose {
                image, image_user, ..
            } => (*image, *image_user),
        };

        let remote_user = match resolve_remote_user(
            common.remote_user.as_deref(),
            common.container_user.as_deref(),
            image_user,
        ) {
            RemoteUserResolution::Update { user } => user,
            _ => return None,
        };

        let folder_tag = crate::docker::image_tag(cwd);
        let uid_tag = if base_image.starts_with(&folder_tag) {
            format!("{base_image}-uid")
        } else {
            format!("{folder_tag}-uid")
        };
        let image_user_str = if image_user.is_empty() {
            "root"
        } else {
            image_user
        }
        .to_string();

        Some(match ctx {
            UidContext::Single { .. } => UidUpdate::Single {
                uid_tag,
                remote_user,
                new_uid: host_uid,
                new_gid: host_gid,
                image_user: image_user_str,
            },
            UidContext::Compose {
                override_content,
                service,
                ..
            } => {
                let override_content = override_content.replacen(
                    &format!("  '{service}':\n"),
                    &format!("  '{service}':\n    image: {uid_tag}\n"),
                    1,
                );
                UidUpdate::Compose {
                    uid_tag,
                    remote_user,
                    new_uid: host_uid,
                    new_gid: host_gid,
                    image_user: image_user_str,
                    override_content,
                }
            }
        })
    }

    pub fn uid_tag(&self) -> &str {
        match self {
            UidUpdate::Single { uid_tag, .. } | UidUpdate::Compose { uid_tag, .. } => uid_tag,
        }
    }
}

#[derive(Debug, PartialEq)]
enum RemoteUserResolution {
    Update { user: String },
    Root,
    Numeric,
}

fn resolve_remote_user(
    config_user: Option<&str>,
    container_user: Option<&str>,
    image_user: &str,
) -> RemoteUserResolution {
    let user = config_user.or(container_user).unwrap_or(image_user);
    let user = if user.is_empty() { "root" } else { user };

    if user == "root" {
        return RemoteUserResolution::Root;
    }
    if user.chars().all(|c| c.is_ascii_digit()) {
        return RemoteUserResolution::Numeric;
    }
    RemoteUserResolution::Update {
        user: user.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devcontainer::config::CommonConfig;

    fn common_with_remote_user(remote_user: Option<&str>) -> CommonConfig {
        CommonConfig {
            remote_user: remote_user.map(str::to_string),
            ..empty_common()
        }
    }

    fn empty_common() -> CommonConfig {
        CommonConfig {
            name: None,
            forward_ports: vec![],
            ports_attributes: None,
            other_ports_attributes: None,
            override_command: None,
            initialize_command: None,
            on_create_command: None,
            update_content_command: None,
            post_create_command: None,
            post_start_command: None,
            post_attach_command: None,
            wait_for: None,
            workspace_folder: None,
            mounts: vec![],
            container_env: Default::default(),
            container_user: None,
            init: None,
            privileged: None,
            cap_add: vec![],
            security_opt: vec![],
            remote_env: None,
            remote_user: None,
            update_remote_user_uid: None,
            user_env_probe: None,
            features: Default::default(),
            override_feature_install_order: vec![],
            host_requirements: None,
            customizations: Default::default(),
        }
    }

    // resolve_remote_user tests (internal helper)

    #[test]
    fn when_config_user_is_root_then_returns_root() {
        assert_eq!(
            resolve_remote_user(Some("root"), None, "vscode"),
            RemoteUserResolution::Root
        );
    }

    #[test]
    fn when_config_user_is_numeric_then_returns_numeric() {
        assert_eq!(
            resolve_remote_user(Some("1000"), None, "vscode"),
            RemoteUserResolution::Numeric
        );
    }

    #[test]
    fn when_config_user_is_named_then_returns_update() {
        assert_eq!(
            resolve_remote_user(Some("vscode"), None, "root"),
            RemoteUserResolution::Update {
                user: "vscode".to_string()
            }
        );
    }

    #[test]
    fn when_no_config_user_and_container_user_is_root_then_returns_root() {
        assert_eq!(
            resolve_remote_user(None, Some("root"), "vscode"),
            RemoteUserResolution::Root
        );
    }

    #[test]
    fn when_no_config_user_and_container_user_is_named_then_returns_update() {
        assert_eq!(
            resolve_remote_user(None, Some("vscode"), "root"),
            RemoteUserResolution::Update {
                user: "vscode".to_string()
            }
        );
    }

    #[test]
    fn when_no_config_user_and_no_container_user_and_image_user_is_root_then_returns_root() {
        assert_eq!(
            resolve_remote_user(None, None, "root"),
            RemoteUserResolution::Root
        );
    }

    #[test]
    fn when_no_config_user_and_no_container_user_and_image_user_is_named_then_returns_update() {
        assert_eq!(
            resolve_remote_user(None, None, "vscode"),
            RemoteUserResolution::Update {
                user: "vscode".to_string()
            }
        );
    }

    #[test]
    fn when_all_empty_then_returns_root() {
        assert_eq!(
            resolve_remote_user(None, None, ""),
            RemoteUserResolution::Root
        );
    }

    #[test]
    fn when_config_user_takes_priority_over_container_user() {
        assert_eq!(
            resolve_remote_user(Some("alice"), Some("root"), "root"),
            RemoteUserResolution::Update {
                user: "alice".to_string()
            }
        );
    }

    #[test]
    fn when_container_user_takes_priority_over_image_user() {
        assert_eq!(
            resolve_remote_user(None, Some("bob"), "root"),
            RemoteUserResolution::Update {
                user: "bob".to_string()
            }
        );
    }

    #[test]
    fn when_no_config_user_and_container_user_is_numeric_then_returns_numeric() {
        assert_eq!(
            resolve_remote_user(None, Some("1000"), "vscode"),
            RemoteUserResolution::Numeric
        );
    }

    #[test]
    fn when_no_config_user_and_no_container_user_and_image_user_is_numeric_then_returns_numeric() {
        assert_eq!(
            resolve_remote_user(None, None, "1000"),
            RemoteUserResolution::Numeric
        );
    }

    #[test]
    fn when_update_remote_user_uid_false_then_resolve_returns_none() {
        let common = CommonConfig {
            update_remote_user_uid: Some(false),
            remote_user: Some("vscode".to_string()),
            ..empty_common()
        };
        let result = UidUpdate::resolve(
            UidContext::Single {
                base_image: "myimage",
                image_user: "root",
            },
            &common,
            1000,
            1000,
            Path::new("/home/user/proj"),
        );
        assert!(result.is_none());
    }

    #[test]
    fn when_single_with_root_user_then_resolve_returns_none() {
        let common = common_with_remote_user(Some("root"));
        let result = UidUpdate::resolve(
            UidContext::Single {
                base_image: "myimage",
                image_user: "root",
            },
            &common,
            1000,
            1000,
            Path::new("/home/user/proj"),
        );
        assert!(result.is_none());
    }

    #[test]
    fn when_single_with_named_user_then_resolve_returns_single_with_uid_tag() {
        let cwd = Path::new("/home/user/myproject");
        let common = common_with_remote_user(Some("vscode"));
        let result = UidUpdate::resolve(
            UidContext::Single {
                base_image: "someimage:latest",
                image_user: "root",
            },
            &common,
            1000,
            1001,
            cwd,
        );
        let update = result.expect("expected Some");
        let folder_tag = crate::docker::image_tag(cwd);
        assert_eq!(update.uid_tag(), format!("{folder_tag}-uid"));
        let UidUpdate::Single {
            remote_user,
            new_uid,
            new_gid,
            image_user,
            ..
        } = update
        else {
            panic!("expected Single variant");
        };
        assert_eq!(remote_user, "vscode");
        assert_eq!(new_uid, 1000);
        assert_eq!(new_gid, 1001);
        assert_eq!(image_user, "root");
    }

    #[test]
    fn when_single_base_image_matches_folder_tag_then_uid_tag_appends_suffix() {
        let cwd = Path::new("/home/user/myproject");
        let folder_tag = crate::docker::image_tag(cwd);
        let base_image = format!("{folder_tag}:latest");
        let common = common_with_remote_user(Some("vscode"));
        let result = UidUpdate::resolve(
            UidContext::Single {
                base_image: &base_image,
                image_user: "root",
            },
            &common,
            1000,
            1000,
            cwd,
        );
        let update = result.expect("expected Some");
        assert_eq!(update.uid_tag(), format!("{base_image}-uid"));
    }

    #[test]
    fn when_compose_with_named_user_then_resolve_returns_compose_with_override() {
        let cwd = Path::new("/home/user/myproject");
        let common = common_with_remote_user(Some("vscode"));
        let override_content = "services:\n  'app':\n    volumes: []\n";
        let result = UidUpdate::resolve(
            UidContext::Compose {
                override_content,
                service: "app",
                image: "someimage:latest",
                image_user: "root",
            },
            &common,
            1000,
            1001,
            cwd,
        );
        let update = result.expect("expected Some");
        let folder_tag = crate::docker::image_tag(cwd);
        let expected_uid_tag = format!("{folder_tag}-uid");
        assert_eq!(update.uid_tag(), expected_uid_tag);
        let UidUpdate::Compose {
            override_content,
            remote_user,
            new_uid,
            new_gid,
            ..
        } = update
        else {
            panic!("expected Compose variant");
        };
        assert_eq!(remote_user, "vscode");
        assert_eq!(new_uid, 1000);
        assert_eq!(new_gid, 1001);
        assert!(
            override_content.contains(&format!("    image: {expected_uid_tag}\n")),
            "override_content missing image injection: {override_content:?}"
        );
    }

    #[test]
    fn when_compose_with_root_user_then_resolve_returns_none() {
        let common = common_with_remote_user(Some("root"));
        let result = UidUpdate::resolve(
            UidContext::Compose {
                override_content: "services:\n  'app':\n",
                service: "app",
                image: "someimage",
                image_user: "root",
            },
            &common,
            1000,
            1000,
            Path::new("/home/user/proj"),
        );
        assert!(result.is_none());
    }

    #[test]
    fn when_image_user_is_empty_then_image_user_field_is_root() {
        let cwd = Path::new("/home/user/proj");
        let common = common_with_remote_user(Some("vscode"));
        let result = UidUpdate::resolve(
            UidContext::Single {
                base_image: "myimage",
                image_user: "",
            },
            &common,
            1000,
            1000,
            cwd,
        );
        let update = result.expect("expected Some");
        let UidUpdate::Single { image_user, .. } = update else {
            panic!("expected Single variant");
        };
        assert_eq!(image_user, "root");
    }
}
