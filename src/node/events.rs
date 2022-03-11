use super::*;

pub(super) struct EventListeners<T: Clone> {
    listeners: Mutex<Vec<Sender<T>>>,
}

impl<T: Clone> EventListeners<T> {
    pub(super) fn new() -> Self {
        Self {
            listeners: Mutex::new(Vec::new()),
        }
    }

    pub(super) async fn event(&self, event: T) {
        let mut listeners = self.listeners.lock().await;
        for i in (0..listeners.len()).rev() {
            // TODO: check if we could optimize by avoiding cloning the last event
            if listeners[i].send(event.clone()).await.is_err() {
                listeners.remove(i);
            }
        }
    }

    pub async fn create_listener(&mut self) -> Receiver<T> {
        let mut listeners = self.listeners.lock().await;
        let (sender, receiver) = async_channel::unbounded();
        listeners.push(sender);
        receiver
    }
}
