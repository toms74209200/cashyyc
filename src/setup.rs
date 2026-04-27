use crate::devcontainer::{
    CommonConfig, ComposeArgs, DevcontainerConfig, DockerfileBuildConfig, DockerfileConfig,
    ImageConfig, compose_args, container_run_options, expand_variables,
};
use crate::docker;
use std::path::Path;

pub struct ContainerSetup {
    pub base_image: String,
    pub run_args: Vec<String>,
    pub override_command: Option<bool>,
}

pub enum ContainerTarget {
    Single(ContainerSetup),
    Compose(ComposeArgs),
}

pub fn from_config(
    config: &DevcontainerConfig,
    cwd: &Path,
    config_path: &Path,
    config_dir: &Path,
) -> ContainerTarget {
    match config {
        DevcontainerConfig::Image(c) => ContainerTarget::Single(from_image(c, cwd, config_path)),
        DevcontainerConfig::Dockerfile(c) => {
            ContainerTarget::Single(from_dockerfile(c, cwd, config_path))
        }
        DevcontainerConfig::DockerfileBuild(c) => {
            ContainerTarget::Single(from_dockerfile_build(c, cwd, config_path))
        }
        DevcontainerConfig::DockerCompose(c) => {
            ContainerTarget::Compose(compose_args(c, cwd, config_dir))
        }
    }
}

pub fn from_image(c: &ImageConfig, cwd: &Path, config_path: &Path) -> ContainerSetup {
    ContainerSetup {
        base_image: c.image.clone(),
        run_args: run_args_for(
            &c.common,
            &c.run_args,
            c.workspace_mount.as_deref(),
            cwd,
            config_path,
        ),
        override_command: c.common.override_command,
    }
}

pub fn from_dockerfile(c: &DockerfileConfig, cwd: &Path, config_path: &Path) -> ContainerSetup {
    ContainerSetup {
        base_image: docker::image_tag(cwd),
        run_args: run_args_for(
            &c.common,
            &c.run_args,
            c.workspace_mount.as_deref(),
            cwd,
            config_path,
        ),
        override_command: c.common.override_command,
    }
}

pub fn from_dockerfile_build(
    c: &DockerfileBuildConfig,
    cwd: &Path,
    config_path: &Path,
) -> ContainerSetup {
    ContainerSetup {
        base_image: docker::image_tag(cwd),
        run_args: run_args_for(
            &c.common,
            &c.run_args,
            c.workspace_mount.as_deref(),
            cwd,
            config_path,
        ),
        override_command: c.common.override_command,
    }
}

fn run_args_for(
    common: &CommonConfig,
    run_args: &[String],
    raw_workspace_mount: Option<&str>,
    cwd: &Path,
    config_path: &Path,
) -> Vec<String> {
    let workspace_folder = common.workspace_folder.clone().unwrap_or_else(|| {
        format!(
            "/workspaces/{}",
            cwd.file_name().unwrap_or_default().to_string_lossy()
        )
    });
    let workspace_mount = raw_workspace_mount.map(|m| expand_variables(m, cwd, &workspace_folder));
    container_run_options(
        common,
        run_args,
        workspace_mount.as_deref(),
        cwd,
        config_path,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devcontainer::BuildConfig;
    use std::collections::HashMap;

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
            container_env: HashMap::new(),
            container_user: None,
            init: None,
            privileged: None,
            cap_add: vec![],
            security_opt: vec![],
            remote_env: None,
            remote_user: None,
            update_remote_user_uid: None,
            user_env_probe: None,
            features: HashMap::new(),
            override_feature_install_order: vec![],
            host_requirements: None,
            customizations: HashMap::new(),
        }
    }

    fn empty_image_config(image: &str) -> ImageConfig {
        ImageConfig {
            image: image.to_string(),
            app_port: None,
            run_args: vec![],
            workspace_mount: None,
            shutdown_action: None,
            common: empty_common(),
        }
    }

    fn empty_dockerfile_config() -> DockerfileConfig {
        DockerfileConfig {
            docker_file: "Dockerfile".to_string(),
            context: None,
            build: None,
            app_port: None,
            run_args: vec![],
            workspace_mount: None,
            shutdown_action: None,
            common: empty_common(),
        }
    }

    fn empty_dockerfile_build_config() -> DockerfileBuildConfig {
        DockerfileBuildConfig {
            build: BuildConfig {
                dockerfile: None,
                context: None,
                target: None,
                args: HashMap::new(),
                cache_from: None,
                options: vec![],
            },
            app_port: None,
            run_args: vec![],
            workspace_mount: None,
            shutdown_action: None,
            common: empty_common(),
        }
    }

    #[test]
    fn when_from_image_then_base_image_is_image_field() {
        let c = empty_image_config("ubuntu:22.04");
        let setup = from_image(
            &c,
            Path::new("/project"),
            Path::new("/project/.devcontainer/devcontainer.json"),
        );
        assert_eq!(setup.base_image, "ubuntu:22.04");
    }

    #[test]
    fn when_from_dockerfile_then_base_image_is_image_tag() {
        let c = empty_dockerfile_config();
        let cwd = Path::new("/home/user/myproject");
        let setup = from_dockerfile(
            &c,
            cwd,
            Path::new("/home/user/myproject/.devcontainer/devcontainer.json"),
        );
        assert_eq!(setup.base_image, docker::image_tag(cwd));
    }

    #[test]
    fn when_from_dockerfile_build_then_base_image_is_image_tag() {
        let c = empty_dockerfile_build_config();
        let cwd = Path::new("/home/user/myproject");
        let setup = from_dockerfile_build(
            &c,
            cwd,
            Path::new("/home/user/myproject/.devcontainer/devcontainer.json"),
        );
        assert_eq!(setup.base_image, docker::image_tag(cwd));
    }

    #[test]
    fn when_from_image_then_run_args_include_detach_flag() {
        let c = empty_image_config("ubuntu:22.04");
        let setup = from_image(
            &c,
            Path::new("/project"),
            Path::new("/project/.devcontainer/devcontainer.json"),
        );
        assert!(setup.run_args.contains(&"-d".to_string()));
    }

    #[test]
    fn when_from_image_with_workspace_mount_then_run_args_expand_variables() {
        let mut c = empty_image_config("ubuntu:22.04");
        c.workspace_mount =
            Some("type=bind,source=${localWorkspaceFolder},target=/workspace".to_string());
        let cwd = Path::new("/home/user/myproject");
        let setup = from_image(
            &c,
            cwd,
            Path::new("/home/user/myproject/.devcontainer/devcontainer.json"),
        );
        let mount_idx = setup.run_args.iter().position(|a| a == "--mount").unwrap();
        assert_eq!(
            setup.run_args[mount_idx + 1],
            "type=bind,source=/home/user/myproject,target=/workspace"
        );
    }

    #[test]
    fn when_from_config_with_image_then_returns_single() {
        let c = empty_image_config("ubuntu:22.04");
        let target = from_config(
            &DevcontainerConfig::Image(c),
            Path::new("/project"),
            Path::new("/project/.devcontainer/devcontainer.json"),
            Path::new("/project/.devcontainer"),
        );
        assert!(matches!(target, ContainerTarget::Single(_)));
    }

    #[test]
    fn when_from_config_with_dockerfile_then_returns_single() {
        let target = from_config(
            &DevcontainerConfig::Dockerfile(empty_dockerfile_config()),
            Path::new("/project"),
            Path::new("/project/.devcontainer/devcontainer.json"),
            Path::new("/project/.devcontainer"),
        );
        assert!(matches!(target, ContainerTarget::Single(_)));
    }

    #[test]
    fn when_from_config_with_dockerfile_build_then_returns_single() {
        let target = from_config(
            &DevcontainerConfig::DockerfileBuild(empty_dockerfile_build_config()),
            Path::new("/project"),
            Path::new("/project/.devcontainer/devcontainer.json"),
            Path::new("/project/.devcontainer"),
        );
        assert!(matches!(target, ContainerTarget::Single(_)));
    }

    #[test]
    fn when_from_image_then_override_command_passes_through() {
        let mut c = empty_image_config("ubuntu:22.04");
        c.common.override_command = Some(false);
        let setup = from_image(
            &c,
            Path::new("/project"),
            Path::new("/project/.devcontainer/devcontainer.json"),
        );
        assert_eq!(setup.override_command, Some(false));
    }
}
