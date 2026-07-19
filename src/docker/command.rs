use crate::error::Error;

pub struct CmdOutput {
    pub success: bool,
    pub status: String,
    pub stdout: String,
    pub stderr: String,
}

pub trait Docker {
    fn stop(&mut self, id: &str) -> Result<bool, Error>;
    fn remove(&mut self, id: &str) -> Result<bool, Error>;
    fn start(&mut self, id: &str) -> Result<bool, Error>;
    fn ps_ids(&mut self, filters: &[String], all_states: bool) -> Result<CmdOutput, Error>;
    fn inspect(&mut self, ids: &[String]) -> Result<CmdOutput, Error>;
    fn inspect_format(&mut self, target: &str, format: &str) -> Result<CmdOutput, Error>;
    fn image_config(&mut self, image: &str) -> Result<CmdOutput, Error>;
    fn pull_streamed(&mut self, image: &str) -> Result<bool, Error>;
    fn build_streamed(&mut self, build_args: &[String]) -> Result<bool, Error>;
    fn run_container(&mut self, run_args: &[String]) -> Result<CmdOutput, Error>;
    fn exec_interactive(&mut self, exec_args: &[String]) -> Result<bool, Error>;
    fn exec_capture(&mut self, exec_args: &[String]) -> Result<CmdOutput, Error>;
    fn exec_group(&mut self, argvs: &[Vec<String>], wait: bool, label: &str)
    -> Result<bool, Error>;
    fn compose(&mut self, args: &[String]) -> Result<bool, Error>;
    fn compose_config_json(&mut self, global_args: &[String]) -> Result<CmdOutput, Error>;
    fn compose_build_streamed(&mut self, args: &[String]) -> Result<bool, Error>;
}
