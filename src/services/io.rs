use crate::imports::*;

/*
    First pass at a built-in extension for CLI interaction. Subject to change, this is a bit of an experiment right now.
*/
pub struct StdoutInteraction;

#[async_trait]
impl PipelineIO for StdoutInteraction {
    async fn notify(&self, message: &str) -> Result<()> {
        println!("{message}");
        Ok(())
    }

    async fn prompt(&self, message: &str) -> Result<Option<String>> {
        println!("{message}");
        let mut input = String::new();
        std::io::stdin().read_line(&mut input)?;
        Ok(match input.trim() {
            "" => None,
            _ => Some(input.trim().to_string()),
        })
    }
}

use crate::imports::*;
use std::sync::mpsc;

pub struct ChannelInteraction {
    sender: mpsc::Sender<Vec<u8>>,
    receiver: mpsc::Receiver<Vec<u8>>,
}
