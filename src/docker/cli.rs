use super::{CmdOutput, Docker};
use crate::err;
use crate::error::Result;
use std::process::Stdio;

pub struct DockerCli;

impl DockerCli {
    fn output(&mut self, args: &[String]) -> Result<CmdOutput> {
        let output = std::process::Command::new("docker")
            .args(args)
            .output()
            .map_err(|e| err!("Failed to run docker: {e}"))?;
        Ok(CmdOutput {
            success: output.status.success(),
            status: output.status.to_string(),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        })
    }

    fn status(&mut self, args: &[String]) -> Result<bool> {
        std::process::Command::new("docker")
            .args(args)
            .status()
            .map(|s| s.success())
            .map_err(|e| err!("Failed to run docker: {e}"))
    }

    fn streamed(&mut self, args: &[String]) -> Result<bool> {
        let mut child = std::process::Command::new("docker")
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| err!("Failed to run docker: {e}"))?;
        let status =
            crate::tui::build_log(&mut child).map_err(|e| err!("Failed to run docker: {e}"))?;
        Ok(status.success())
    }
}

fn with_subcommand(subcommand: &[&str], args: &[String]) -> Vec<String> {
    let mut all: Vec<String> = subcommand.iter().map(|s| s.to_string()).collect();
    all.extend(args.iter().cloned());
    all
}

impl Docker for DockerCli {
    fn stop(&mut self, id: &str) -> Result<bool> {
        self.status(&["stop".to_string(), id.to_string()])
    }

    fn remove(&mut self, id: &str) -> Result<bool> {
        self.status(&["rm".to_string(), "-f".to_string(), id.to_string()])
    }

    fn start(&mut self, id: &str) -> Result<bool> {
        self.status(&["start".to_string(), id.to_string()])
    }

    fn ps_ids(&mut self, filters: &[String], all_states: bool) -> Result<CmdOutput> {
        let mut args = vec!["ps".to_string()];
        if all_states {
            args.push("-a".to_string());
        }
        for filter in filters {
            args.push("--filter".to_string());
            args.push(filter.clone());
        }
        args.extend(["--format".to_string(), "{{.ID}}".to_string()]);
        self.output(&args)
    }

    fn inspect(&mut self, ids: &[String]) -> Result<CmdOutput> {
        self.output(&with_subcommand(&["inspect"], ids))
    }

    fn inspect_format(&mut self, target: &str, format: &str) -> Result<CmdOutput> {
        self.output(&["inspect", "--format", format, target].map(String::from))
    }

    fn image_config(&mut self, image: &str) -> Result<CmdOutput> {
        self.output(&["image", "inspect", "--format", "{{json .Config}}", image].map(String::from))
    }

    fn pull_streamed(&mut self, image: &str) -> Result<bool> {
        self.streamed(&["pull".to_string(), image.to_string()])
    }

    fn build_streamed(&mut self, build_args: &[String]) -> Result<bool> {
        self.streamed(&with_subcommand(&["build"], build_args))
    }

    fn run_container(&mut self, run_args: &[String]) -> Result<CmdOutput> {
        self.output(&with_subcommand(&["run"], run_args))
    }

    fn copy_from_image(&mut self, image: &str, src: &str, dest: &str) -> Result<CmdOutput> {
        let created =
            self.output(&["create", "--entrypoint", "sh", image, "-c", "true"].map(String::from))?;
        if !created.success {
            return Ok(created);
        }
        let Some(id) = super::parse_container_id(&created.stdout) else {
            return Ok(CmdOutput {
                success: false,
                ..created
            });
        };
        let copied = self.output(&["cp".to_string(), format!("{id}:{src}"), dest.to_string()]);
        let _ = self.output(&["rm".to_string(), "-f".to_string(), id]);
        copied
    }

    fn exec_interactive(&mut self, exec_args: &[String]) -> Result<bool> {
        self.status(&with_subcommand(&["exec"], exec_args))
    }

    fn exec_capture(&mut self, exec_args: &[String]) -> Result<CmdOutput> {
        self.output(&with_subcommand(&["exec"], exec_args))
    }

    fn exec_group(&mut self, argvs: &[Vec<String>], wait: bool, label: &str) -> Result<bool> {
        let mut children = Vec::new();
        for args in argvs {
            children.push(
                std::process::Command::new("docker")
                    .arg("exec")
                    .args(args)
                    .spawn()
                    .map_err(|e| err!("Failed to run docker: {e}"))?,
            );
        }
        if wait {
            for child in &mut children {
                if !child
                    .wait()
                    .map_err(|e| err!("Failed to wait for {label}: {e}"))?
                    .success()
                {
                    return Ok(false);
                }
            }
        }
        Ok(true)
    }

    fn compose(&mut self, args: &[String]) -> Result<bool> {
        self.status(&with_subcommand(&["compose"], args))
    }

    fn compose_config_json(&mut self, global_args: &[String]) -> Result<CmdOutput> {
        let mut args = with_subcommand(&["compose"], global_args);
        args.extend(["config", "--format", "json"].map(String::from));
        self.output(&args)
    }

    fn compose_build_streamed(&mut self, args: &[String]) -> Result<bool> {
        self.streamed(&with_subcommand(&["compose"], args))
    }
}
