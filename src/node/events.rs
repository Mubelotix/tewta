// Copyright (c) 2022  Mubelotix <mubelotix@gmail.com>
// Program licensed under GNU AGPL v3 or later. See the LICENSE file for details.

use super::*;

pub struct EventListeners<T: Clone> {
    listeners: Mutex<Vec<Sender<T>>>,
}

impl<T: Clone> EventListeners<T> {
    #[allow(clippy::collapsible_if)]
    pub async fn event(&self, event: T) {
        let mut listeners = self.listeners.lock().await;
        if listeners.len() > 1 {
            // We avoid the first element because if there is one, we can spare on clone (the last)
            for i in (1..listeners.len()).rev() {
                if listeners[i].send(event.clone()).await.is_err() {
                    listeners.remove(i);
                }
            }
        }
        if !listeners.is_empty() {
            if listeners[0].send(event.clone()).await.is_err() {
                listeners.remove(0);
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
