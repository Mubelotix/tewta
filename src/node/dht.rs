pub use super::*;

#[derive(Debug, Clone, protocol_derive::Protocol)]
pub struct DhtValue {
    // TODO: Populate the DhtValue struct
}

#[derive(Default)]
pub struct DhtStore {
    table: Mutex<BTreeMap<KeyID, Vec<DhtValue>>>,
}

impl DhtStore {
    pub(super) async fn get(&self, key: &KeyID) -> Option<Vec<DhtValue>> {
        let mut table = self.table.lock().await;
        let values = table.get(key);
        if let Some(values) = values {
            if values.is_empty() {
                table.remove(key);
                return None;
            }
        }
        values.cloned()
    }
}
