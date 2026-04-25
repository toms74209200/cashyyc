use super::config::*;

pub fn parse_config(content: &str) -> Option<DevcontainerConfig> {
    let mut stripped = String::with_capacity(content.len());
    let mut chars = content.chars().peekable();
    let mut in_string = false;

    while let Some(c) = chars.next() {
        match c {
            '"' if in_string => {
                in_string = false;
                stripped.push(c);
            }
            '\\' if in_string => {
                stripped.push(c);
                if let Some(next) = chars.next() {
                    stripped.push(next);
                }
            }
            '"' => {
                in_string = true;
                stripped.push(c);
            }
            '/' if !in_string => match chars.peek() {
                Some('/') => {
                    chars.next();
                    for c in chars.by_ref() {
                        if c == '\n' {
                            stripped.push('\n');
                            break;
                        }
                    }
                }
                Some('*') => {
                    chars.next();
                    while let Some(c) = chars.next() {
                        if c == '*' && chars.peek() == Some(&'/') {
                            chars.next();
                            break;
                        }
                    }
                }
                _ => stripped.push(c),
            },
            _ => stripped.push(c),
        }
    }

    let mut clean = String::with_capacity(stripped.len());
    let mut chars = stripped.chars().peekable();
    let mut in_string = false;
    while let Some(c) = chars.next() {
        match c {
            '\\' if in_string => {
                clean.push(c);
                if let Some(next) = chars.next() {
                    clean.push(next);
                }
            }
            '"' => {
                in_string = !in_string;
                clean.push(c);
            }
            ',' if !in_string => {
                let mut whitespace = String::new();
                while let Some(&w) = chars.peek() {
                    if w.is_ascii_whitespace() {
                        whitespace.push(chars.next().unwrap());
                    } else {
                        break;
                    }
                }
                if !matches!(chars.peek(), Some('}') | Some(']')) {
                    clean.push(c);
                }
                clean.push_str(&whitespace);
            }
            _ => clean.push(c),
        }
    }

    let value: serde_json::Value = serde_json::from_str(&clean).ok()?;

    match (
        value.get("dockerComposeFile"),
        value.get("dockerFile"),
        value.get("build"),
        value.get("image"),
    ) {
        (Some(_), _, _, _) => serde_json::from_value(value)
            .ok()
            .map(DevcontainerConfig::DockerCompose),
        (_, Some(_), _, _) => serde_json::from_value(value)
            .ok()
            .map(DevcontainerConfig::Dockerfile),
        (_, _, Some(_), _) => serde_json::from_value(value)
            .ok()
            .map(DevcontainerConfig::DockerfileBuild),
        (_, _, _, Some(_)) => serde_json::from_value(value)
            .ok()
            .map(DevcontainerConfig::Image),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn when_parse_config_with_trailing_comma_in_object_then_succeeds() {
        assert!(matches!(
            parse_config("{\"image\": \"rust:latest\",}"),
            Some(DevcontainerConfig::Image(_))
        ));
    }

    #[test]
    fn when_parse_config_with_trailing_comma_in_array_then_succeeds() {
        assert!(matches!(
            parse_config(
                r#"{"image": "rust:latest", "capAdd": ["SYS_PTRACE",]}"#
            ),
            Some(DevcontainerConfig::Image(ImageConfig {
                common: CommonConfig { cap_add: ref ca, .. },
                ..
            })) if ca == &vec!["SYS_PTRACE".to_string()]
        ));
    }

    #[test]
    fn when_parse_config_with_comma_in_string_value_then_preserves_value() {
        assert!(matches!(
            parse_config(r#"{"image": "registry.example.com,backup:latest"}"#),
            Some(DevcontainerConfig::Image(ImageConfig { image: ref i, .. })) if i == "registry.example.com,backup:latest"
        ));
    }

    #[test]
    fn when_parse_config_with_double_slash_in_string_value_then_preserves_value() {
        assert!(matches!(
            parse_config(r#"{"image": "http://registry.example.com"}"#),
            Some(DevcontainerConfig::Image(ImageConfig { image: ref i, .. })) if i == "http://registry.example.com"
        ));
    }

    #[test]
    fn when_parse_config_with_block_comment_marker_in_string_value_then_preserves_value() {
        assert!(matches!(
            parse_config(r#"{"image": "value /* not a comment */ end"}"#),
            Some(DevcontainerConfig::Image(ImageConfig { image: ref i, .. })) if i == "value /* not a comment */ end"
        ));
    }

    #[test]
    fn when_parse_config_with_escaped_backslash_in_string_then_succeeds() {
        assert!(matches!(
            parse_config("{\"image\": \"rust:latest\", \"name\": \"test\\\\value\"}"),
            Some(DevcontainerConfig::Image(ImageConfig {
                common: CommonConfig { name: Some(ref n), .. },
                ..
            })) if n == "test\\value"
        ));
    }

    #[test]
    fn when_parse_config_with_escaped_quote_in_string_then_preserves_name() {
        assert!(matches!(
            parse_config("{\"image\": \"rust:latest\", \"name\": \"say \\\"hello\\\"\"}"),
            Some(DevcontainerConfig::Image(ImageConfig {
                common: CommonConfig { name: Some(ref n), .. },
                ..
            })) if n == "say \"hello\""
        ));
    }

    #[test]
    fn when_parse_config_with_multiline_block_comment_then_succeeds() {
        assert!(parse_config("{\n/* line1\n   line2 */\n\"image\": \"rust:latest\"\n}").is_some());
    }

    #[test]
    fn when_parse_config_with_empty_string_then_returns_none() {
        assert_eq!(parse_config(""), None);
    }

    #[test]
    fn when_parse_config_with_lone_slash_outside_string_then_returns_none() {
        assert_eq!(parse_config("{\"image\": \"rust:latest\"} /"), None);
    }

    #[test]
    fn when_parse_config_with_no_discriminating_field_then_returns_none() {
        assert_eq!(parse_config(r#"{"name": "x"}"#), None);
    }

    #[test]
    fn when_parse_config_with_build_cache_from_as_string_then_returns_single_element_vec() {
        assert!(matches!(
            parse_config(r#"{"build": {"dockerfile": "Dockerfile", "cacheFrom": "myimage:latest"}}"#),
            Some(DevcontainerConfig::DockerfileBuild(DockerfileBuildConfig {
                build: BuildConfig { cache_from: Some(ref v), .. },
                ..
            })) if *v == vec!["myimage:latest".to_string()]
        ));
    }

    #[test]
    fn when_parse_config_with_build_cache_from_as_array_then_parses_all_elements() {
        assert!(matches!(
            parse_config(r#"{"build": {"dockerfile": "Dockerfile", "cacheFrom": ["image1:latest", "image2:latest"]}}"#),
            Some(DevcontainerConfig::DockerfileBuild(DockerfileBuildConfig {
                build: BuildConfig { cache_from: Some(ref v), .. },
                ..
            })) if *v == vec!["image1:latest".to_string(), "image2:latest".to_string()]
        ));
    }

    #[test]
    fn when_parse_config_with_build_cache_from_null_then_is_none() {
        assert!(matches!(
            parse_config(r#"{"build": {"dockerfile": "Dockerfile", "cacheFrom": null}}"#),
            Some(DevcontainerConfig::DockerfileBuild(DockerfileBuildConfig {
                build: BuildConfig {
                    cache_from: None,
                    ..
                },
                ..
            }))
        ));
    }

    #[test]
    fn when_parse_config_with_build_cache_from_absent_then_is_none() {
        assert!(matches!(
            parse_config(r#"{"build": {"dockerfile": "Dockerfile"}}"#),
            Some(DevcontainerConfig::DockerfileBuild(DockerfileBuildConfig {
                build: BuildConfig {
                    cache_from: None,
                    ..
                },
                ..
            }))
        ));
    }

    #[test]
    fn when_parse_config_with_dockerfilebuild_then_parses_all_fields() {
        let json = r#"{
            "build": {
                "dockerfile": "Dockerfile",
                "context": ".",
                "target": "dev",
                "args": {"KEY": "value"},
                "options": ["--no-cache"]
            },
            "appPort": 3000,
            "runArgs": ["--rm"],
            "workspaceMount": "source=${localWorkspaceFolder},target=/workspace,type=bind",
            "shutdownAction": "none"
        }"#;
        assert!(matches!(
            parse_config(json),
            Some(DevcontainerConfig::DockerfileBuild(DockerfileBuildConfig {
                build: BuildConfig {
                    dockerfile: Some(ref d),
                    context: Some(ref ctx),
                    target: Some(ref t),
                    options: ref opts,
                    args: ref a,
                    ..
                },
                app_port: Some(_),
                run_args: ref ra,
                workspace_mount: Some(_),
                shutdown_action: Some(ref sa),
                ..
            })) if d == "Dockerfile"
                && ctx == "."
                && t == "dev"
                && sa == "none"
                && ra == &vec!["--rm".to_string()]
                && opts == &vec!["--no-cache".to_string()]
                && a.get("KEY") == Some(&"value".to_string())
        ));
    }

    #[test]
    fn when_parse_config_with_docker_compose_run_services_then_parses_correctly() {
        let json = r#"{
            "dockerComposeFile": "docker-compose.yml",
            "service": "app",
            "workspaceFolder": "/workspace",
            "runServices": ["db", "cache"],
            "shutdownAction": "stopCompose"
        }"#;
        assert!(matches!(
            parse_config(json),
            Some(DevcontainerConfig::DockerCompose(DockerComposeConfig {
                run_services: ref rs,
                shutdown_action: Some(ref sa),
                workspace_folder: ref wf,
                ..
            })) if rs == &vec!["db".to_string(), "cache".to_string()]
                && sa == "stopCompose"
                && wf == "/workspace"
        ));
    }

    #[test]
    fn when_parse_config_with_user_env_probe_none_then_parses_correctly() {
        assert!(matches!(
            parse_config(r#"{"image": "rust:latest", "userEnvProbe": "none"}"#),
            Some(DevcontainerConfig::Image(ImageConfig {
                common: CommonConfig {
                    user_env_probe: Some(UserEnvProbe::None),
                    ..
                },
                ..
            }))
        ));
    }

    #[test]
    fn when_parse_config_with_user_env_probe_login_interactive_shell_then_parses_correctly() {
        assert!(matches!(
            parse_config(r#"{"image": "rust:latest", "userEnvProbe": "loginInteractiveShell"}"#),
            Some(DevcontainerConfig::Image(ImageConfig {
                common: CommonConfig {
                    user_env_probe: Some(UserEnvProbe::LoginInteractiveShell),
                    ..
                },
                ..
            }))
        ));
    }

    #[test]
    fn when_parse_config_with_user_env_probe_interactive_shell_then_parses_correctly() {
        assert!(matches!(
            parse_config(r#"{"image": "rust:latest", "userEnvProbe": "interactiveShell"}"#),
            Some(DevcontainerConfig::Image(ImageConfig {
                common: CommonConfig {
                    user_env_probe: Some(UserEnvProbe::InteractiveShell),
                    ..
                },
                ..
            }))
        ));
    }

    #[test]
    fn when_parse_config_with_user_env_probe_login_shell_then_parses_correctly() {
        assert!(matches!(
            parse_config(r#"{"image": "rust:latest", "userEnvProbe": "loginShell"}"#),
            Some(DevcontainerConfig::Image(ImageConfig {
                common: CommonConfig {
                    user_env_probe: Some(UserEnvProbe::LoginShell),
                    ..
                },
                ..
            }))
        ));
    }

    #[test]
    fn when_parse_config_with_wait_for_initialize_command_then_parses_correctly() {
        assert!(matches!(
            parse_config(r#"{"image": "rust:latest", "waitFor": "initializeCommand"}"#),
            Some(DevcontainerConfig::Image(ImageConfig {
                common: CommonConfig {
                    wait_for: Some(WaitFor::InitializeCommand),
                    ..
                },
                ..
            }))
        ));
    }

    #[test]
    fn when_parse_config_with_wait_for_on_create_command_then_parses_correctly() {
        assert!(matches!(
            parse_config(r#"{"image": "rust:latest", "waitFor": "onCreateCommand"}"#),
            Some(DevcontainerConfig::Image(ImageConfig {
                common: CommonConfig {
                    wait_for: Some(WaitFor::OnCreateCommand),
                    ..
                },
                ..
            }))
        ));
    }

    #[test]
    fn when_parse_config_with_wait_for_update_content_command_then_parses_correctly() {
        assert!(matches!(
            parse_config(r#"{"image": "rust:latest", "waitFor": "updateContentCommand"}"#),
            Some(DevcontainerConfig::Image(ImageConfig {
                common: CommonConfig {
                    wait_for: Some(WaitFor::UpdateContentCommand),
                    ..
                },
                ..
            }))
        ));
    }

    #[test]
    fn when_parse_config_with_wait_for_post_create_command_then_parses_correctly() {
        assert!(matches!(
            parse_config(r#"{"image": "rust:latest", "waitFor": "postCreateCommand"}"#),
            Some(DevcontainerConfig::Image(ImageConfig {
                common: CommonConfig {
                    wait_for: Some(WaitFor::PostCreateCommand),
                    ..
                },
                ..
            }))
        ));
    }

    #[test]
    fn when_parse_config_with_wait_for_post_start_command_then_parses_correctly() {
        assert!(matches!(
            parse_config(r#"{"image": "rust:latest", "waitFor": "postStartCommand"}"#),
            Some(DevcontainerConfig::Image(ImageConfig {
                common: CommonConfig {
                    wait_for: Some(WaitFor::PostStartCommand),
                    ..
                },
                ..
            }))
        ));
    }

    #[test]
    fn when_parse_config_with_wait_for_post_attach_command_then_parses_correctly() {
        assert!(matches!(
            parse_config(r#"{"image": "rust:latest", "waitFor": "postAttachCommand"}"#),
            Some(DevcontainerConfig::Image(ImageConfig {
                common: CommonConfig {
                    wait_for: Some(WaitFor::PostAttachCommand),
                    ..
                },
                ..
            }))
        ));
    }

    #[test]
    fn when_parse_config_with_container_env_then_parses_correctly() {
        assert!(matches!(
            parse_config(r#"{"image": "rust:latest", "containerEnv": {"RUST_LOG": "debug", "PORT": "8080"}}"#),
            Some(DevcontainerConfig::Image(ImageConfig {
                common: CommonConfig { container_env: ref e, .. },
                ..
            })) if e.get("RUST_LOG") == Some(&"debug".to_string())
                && e.get("PORT") == Some(&"8080".to_string())
        ));
    }

    #[test]
    fn when_parse_config_with_remote_env_null_value_then_parses_correctly() {
        assert!(matches!(
            parse_config(r#"{"image": "rust:latest", "remoteEnv": {"UNSET_VAR": null, "SET_VAR": "value"}}"#),
            Some(DevcontainerConfig::Image(ImageConfig {
                common: CommonConfig { remote_env: Some(ref e), .. },
                ..
            })) if e.get("UNSET_VAR") == Some(&None)
                && e.get("SET_VAR") == Some(&Some("value".to_string()))
        ));
    }

    #[test]
    fn when_parse_config_with_cap_add_and_security_opt_then_parses_correctly() {
        assert!(matches!(
            parse_config(r#"{"image": "rust:latest", "capAdd": ["SYS_PTRACE"], "securityOpt": ["seccomp=unconfined"]}"#),
            Some(DevcontainerConfig::Image(ImageConfig {
                common: CommonConfig {
                    cap_add: ref ca,
                    security_opt: ref so,
                    ..
                },
                ..
            })) if ca == &vec!["SYS_PTRACE".to_string()]
                && so == &vec!["seccomp=unconfined".to_string()]
        ));
    }

    #[test]
    fn when_parse_config_with_ports_attributes_then_parses_correctly() {
        let json = r#"{
            "image": "rust:latest",
            "portsAttributes": {
                "3000": {"label": "Application", "onAutoForward": "notify", "elevateIfNeeded": false}
            }
        }"#;
        assert!(matches!(
            parse_config(json),
            Some(DevcontainerConfig::Image(ImageConfig {
                common: CommonConfig { ports_attributes: Some(ref attrs), .. },
                ..
            })) if attrs.get("3000").and_then(|a| a.label.as_deref()) == Some("Application")
                && attrs.get("3000").and_then(|a| a.on_auto_forward.as_deref()) == Some("notify")
                && attrs.get("3000").and_then(|a| a.elevate_if_needed) == Some(false)
        ));
    }

    #[test]
    fn when_parse_config_with_host_requirements_then_parses_correctly() {
        assert!(matches!(
            parse_config(r#"{"image": "rust:latest", "hostRequirements": {"cpus": 4, "memory": "8gb", "storage": "32gb"}}"#),
            Some(DevcontainerConfig::Image(ImageConfig {
                common: CommonConfig {
                    host_requirements: Some(HostRequirements {
                        cpus: Some(4),
                        memory: Some(ref m),
                        storage: Some(ref s),
                        ..
                    }),
                    ..
                },
                ..
            })) if m == "8gb" && s == "32gb"
        ));
    }

    #[test]
    fn when_parse_config_with_image_then_returns_image_variant() {
        assert!(matches!(
            parse_config(r#"{"name": "Rust", "image": "mcr.microsoft.com/devcontainers/rust:2-1-trixie"}"#),
            Some(DevcontainerConfig::Image(ImageConfig {
                image: ref i,
                common: CommonConfig { name: Some(ref n), .. },
                ..
            })) if i == "mcr.microsoft.com/devcontainers/rust:2-1-trixie" && n == "Rust"
        ));
    }

    #[test]
    fn when_parse_config_with_docker_file_then_returns_dockerfile_variant() {
        assert!(matches!(
            parse_config(r#"{"name": "Dev", "dockerFile": "Dockerfile"}"#),
            Some(DevcontainerConfig::Dockerfile(DockerfileConfig {
                docker_file: ref df,
                common: CommonConfig { name: Some(ref n), .. },
                ..
            })) if df == "Dockerfile" && n == "Dev"
        ));
    }

    #[test]
    fn when_parse_config_with_build_dockerfile_then_returns_dockerfilebuild_variant() {
        assert!(matches!(
            parse_config(r#"{"build": {"dockerfile": "Dockerfile"}}"#),
            Some(DevcontainerConfig::DockerfileBuild(_))
        ));
    }

    #[test]
    fn when_parse_config_with_docker_compose_file_string_then_returns_dockercompose_variant() {
        assert!(matches!(
            parse_config(r#"{"dockerComposeFile": "docker-compose.yml", "service": "app", "workspaceFolder": "/workspace"}"#),
            Some(DevcontainerConfig::DockerCompose(DockerComposeConfig {
                docker_compose_file: ref dcf,
                service: ref svc,
                ..
            })) if *dcf == vec!["docker-compose.yml".to_string()] && svc == "app"
        ));
    }

    #[test]
    fn when_parse_config_with_docker_compose_file_array_then_returns_dockercompose_variant() {
        assert!(matches!(
            parse_config(r#"{"dockerComposeFile": ["docker-compose.yml", "docker-compose.override.yml"], "service": "app", "workspaceFolder": "/workspace"}"#),
            Some(DevcontainerConfig::DockerCompose(DockerComposeConfig {
                docker_compose_file: ref dcf,
                ..
            })) if *dcf == vec!["docker-compose.yml".to_string(), "docker-compose.override.yml".to_string()]
        ));
    }

    #[test]
    fn when_parse_config_with_common_fields_then_parses_correctly() {
        assert!(matches!(
            parse_config(r#"{"image": "rust:latest", "remoteUser": "vscode", "postCreateCommand": "cargo build", "features": {}}"#),
            Some(DevcontainerConfig::Image(ImageConfig {
                common: CommonConfig {
                    remote_user: Some(ref u),
                    post_create_command: Some(_),
                    ..
                },
                ..
            })) if u == "vscode"
        ));
    }

    #[test]
    fn when_parse_config_with_line_comments_then_succeeds() {
        assert!(
            parse_config("{\n// comment\n\"name\": \"Rust\",\n\"image\": \"rust:latest\"\n}")
                .is_some()
        );
    }

    #[test]
    fn when_parse_config_with_block_comments_then_succeeds() {
        assert!(
            parse_config(r#"{ /* comment */ "name": "Rust", "image": "rust:latest" }"#).is_some()
        );
    }

    #[test]
    fn when_parse_config_with_invalid_json_then_returns_none() {
        assert_eq!(parse_config("{ invalid }"), None);
    }
}
