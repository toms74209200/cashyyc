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
}
