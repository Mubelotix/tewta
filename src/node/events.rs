// Copyright (c) 2022  Mubelotix <Mubelotix@gmail.com>
// Program licensed under GNU AGPL v3 or later. See the LICENSE file for details.

use super::*;

pub(super) struct EventListeners<T: Clone> {
    listeners: Mutex<Vec<Sender<T>>>,
}

impl<T: Clone> EventListeners<T> {
    pub(super) async fn event(&self, event: T) {
        let mut listeners = self.listeners.lock().await;
        for i in (0..listeners.len()).rev() {
            // TODO [#4]: check if we could optimize by avoiding cloning the last event
            if listeners[i].send(event.clone()).await.is_err() {
                listeners.remove(i);
            }
        }
    }

    pub async fn listen(&self) -> Receiver<T> {
        let mut listeners = self.listeners.lock().await;
        let (sender, receiver) = async_channel::unbounded();
        listeners.push(sender);
        receiver
    }
}

impl<T: Clone> Default for EventListeners<T> {
    fn default() -> EventListeners<T> {
        EventListeners {
            listeners: Mutex::new(Vec::new()),
        }
    }
}
