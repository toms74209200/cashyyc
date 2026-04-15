#[derive(Debug, PartialEq)]
pub enum Command {
    Shell { name: Option<String> },
    Help,
    Version,
    Unknown(String),
}

pub fn parse_args(args: &[String]) -> Command {
    match args.len() {
        0 | 1 => Command::Unknown("no command".to_string()),
        2 => match args[1].as_str() {
            "shell" => Command::Shell { name: None },
            "help" | "--help" | "-h" => Command::Help,
            "version" | "--version" | "-V" => Command::Version,
            cmd => Command::Unknown(cmd.to_string()),
        },
        _ => match args[1].as_str() {
            "shell" => Command::Shell {
                name: Some(args[2].clone()),
            },
            "help" | "--help" | "-h" => Command::Help,
            "version" | "--version" | "-V" => Command::Version,
            cmd => Command::Unknown(cmd.to_string()),
        },
    }
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
    fn when_parse_args_with_shell_command_then_returns_shell_with_none() {
        let args = vec!["cyyc".to_string(), "shell".to_string()];
        assert_eq!(parse_args(&args), Command::Shell { name: None });
    }

    #[test]
    fn when_parse_args_with_shell_and_environment_name_then_returns_shell_with_name() {
        let name = generate_random_string(
            8,
            &[
                CharacterType::Lowercase,
                CharacterType::Uppercase,
                CharacterType::Numeric,
            ],
            "",
            &mut urandom(),
        );
        let args = vec!["cyyc".to_string(), "shell".to_string(), name.clone()];
        assert_eq!(parse_args(&args), Command::Shell { name: Some(name) });
    }

    #[test]
    fn when_parse_args_with_unknown_command_then_returns_unknown() {
        let cmd = generate_random_string(
            8,
            &[
                CharacterType::Lowercase,
                CharacterType::Uppercase,
                CharacterType::Numeric,
            ],
            "",
            &mut urandom(),
        );
        let args = vec!["cyyc".to_string(), cmd.clone()];
        assert_eq!(parse_args(&args), Command::Unknown(cmd));
    }

    #[test]
    fn when_parse_args_with_unknown_command_and_extra_arg_then_returns_unknown() {
        let cmd = generate_random_string(
            8,
            &[
                CharacterType::Lowercase,
                CharacterType::Uppercase,
                CharacterType::Numeric,
            ],
            "",
            &mut urandom(),
        );
        let extra = generate_random_string(
            8,
            &[
                CharacterType::Lowercase,
                CharacterType::Uppercase,
                CharacterType::Numeric,
            ],
            "",
            &mut urandom(),
        );
        let args = vec!["cyyc".to_string(), cmd.clone(), extra];
        assert_eq!(parse_args(&args), Command::Unknown(cmd));
    }

    #[test]
    fn when_parse_args_with_program_name_only_then_returns_unknown() {
        let args = vec!["cyyc".to_string()];
        assert_eq!(
            parse_args(&args),
            Command::Unknown("no command".to_string())
        );
    }

    #[test]
    fn when_parse_args_with_help_command_then_returns_help() {
        let args = vec!["cyyc".to_string(), "help".to_string()];
        assert_eq!(parse_args(&args), Command::Help);
    }

    #[test]
    fn when_parse_args_with_help_flag_then_returns_help() {
        let args = vec!["cyyc".to_string(), "--help".to_string()];
        assert_eq!(parse_args(&args), Command::Help);
    }

    #[test]
    fn when_parse_args_with_short_help_flag_then_returns_help() {
        let args = vec!["cyyc".to_string(), "-h".to_string()];
        assert_eq!(parse_args(&args), Command::Help);
    }

    #[test]
    fn when_parse_args_with_help_command_and_extra_arg_then_returns_help() {
        let args = vec!["cyyc".to_string(), "help".to_string(), "extra".to_string()];
        assert_eq!(parse_args(&args), Command::Help);
    }

    #[test]
    fn when_parse_args_with_version_command_then_returns_version() {
        let args = vec!["cyyc".to_string(), "version".to_string()];
        assert_eq!(parse_args(&args), Command::Version);
    }

    #[test]
    fn when_parse_args_with_version_flag_then_returns_version() {
        let args = vec!["cyyc".to_string(), "--version".to_string()];
        assert_eq!(parse_args(&args), Command::Version);
    }

    #[test]
    fn when_parse_args_with_short_version_flag_then_returns_version() {
        let args = vec!["cyyc".to_string(), "-V".to_string()];
        assert_eq!(parse_args(&args), Command::Version);
    }

    #[test]
    fn when_parse_args_with_version_command_and_extra_arg_then_returns_version() {
        let args = vec![
            "cyyc".to_string(),
            "version".to_string(),
            "extra".to_string(),
        ];
        assert_eq!(parse_args(&args), Command::Version);
    }
}
