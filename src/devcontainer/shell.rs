pub fn parse_shell_from_passwd(passwd_line: &str) -> Option<String> {
    let shell = passwd_line.trim().split(':').nth(6)?;
    if shell.is_empty() {
        None
    } else {
        Some(shell.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn when_parse_shell_from_passwd_with_shell_then_returns_some() {
        assert_eq!(
            parse_shell_from_passwd("vscode:x:1000:1000::/home/vscode:/bin/bash"),
            Some("/bin/bash".to_string())
        );
    }

    #[test]
    fn when_parse_shell_from_passwd_with_empty_string_then_returns_none() {
        assert_eq!(parse_shell_from_passwd(""), None);
    }

    #[test]
    fn when_parse_shell_from_passwd_with_insufficient_fields_then_returns_none() {
        assert_eq!(parse_shell_from_passwd("vscode:x:1000:1000"), None);
    }

    #[test]
    fn when_parse_shell_from_passwd_with_empty_shell_field_then_returns_none() {
        assert_eq!(parse_shell_from_passwd("vscode:x:1000:1000:::"), None);
    }
}
