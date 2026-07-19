use crate::devcontainer::jsonc::JsoncError;

#[derive(Debug, PartialEq)]
pub struct Error(String);

impl Error {
    pub fn new(message: impl Into<String>) -> Self {
        Error(message.into())
    }
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error(e.to_string())
    }
}

impl From<JsoncError> for Error {
    fn from(e: JsoncError) -> Self {
        Error(e.to_string())
    }
}

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[macro_export]
macro_rules! err {
    ($($arg:tt)*) => {
        $crate::error::Error::new(format!($($arg)*))
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn when_display_then_returns_message_only() {
        assert_eq!(
            format!("{}", err!("failed to run {}", "docker")),
            "failed to run docker"
        );
    }

    #[test]
    fn when_from_io_error_then_uses_io_error_message() {
        let io = std::io::Error::new(std::io::ErrorKind::NotFound, "missing file");
        assert_eq!(format!("{}", Error::from(io)), "missing file");
    }
}
