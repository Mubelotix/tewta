use super::*;

pub struct CommandReceiver {
    receiver: async_channel::Receiver<Command>,
}

impl CommandReceiver {
    pub fn new() -> (CommandReceiver, async_channel::Sender<Command>) {
        let (sender, receiver) = async_channel::unbounded();
        (CommandReceiver { receiver }, sender)
    }

    pub async fn wait_command(&self) -> Command {
        self.receiver.recv().await.unwrap()
    }
}
