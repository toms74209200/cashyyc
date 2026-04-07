use crate::cli;
use anyhow::{Result, anyhow};

pub fn run(args: Vec<String>) -> Result<()> {
    match cli::parser::parse_args(&args) {
        cli::parser::Command::Shell { name: _ } => {
            let cwd = std::env::current_dir()?;
            let config_path = cwd.join(".devcontainer").join("devcontainer.json");
            let _config = std::fs::read_to_string(&config_path).map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    anyhow!(
                        "Dev container config ({}) not found.",
                        config_path.display()
                    )
                } else {
                    anyhow!("Dev container config ({}): {e}", config_path.display())
                }
            })?;
            Ok(())
        }
        cli::parser::Command::Unknown(msg) => Err(anyhow!(msg)),
    }
}
