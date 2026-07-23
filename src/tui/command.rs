use crate::error::Error;

pub trait Terminal {
    fn select(&mut self, label: &str, items: &[String]) -> Result<Option<usize>, Error>;
    fn multi_select(&mut self, label: &str, items: &[String]) -> Result<Option<Vec<usize>>, Error>;
}
