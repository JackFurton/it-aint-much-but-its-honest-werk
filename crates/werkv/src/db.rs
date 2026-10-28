use std::collections::HashMap;
use std::sync::{Arc, Mutex};

// std Mutex is fine here because the lock is never held across an .await.
#[derive(Clone, Default)]
pub struct Db {
    entries: Arc<Mutex<HashMap<Vec<u8>, Vec<u8>>>>,
}

impl Db {
    pub fn get(&self, key: &[u8]) -> Option<Vec<u8>> {
        self.entries.lock().unwrap().get(key).cloned()
    }

    pub fn set(&self, key: Vec<u8>, value: Vec<u8>) {
        self.entries.lock().unwrap().insert(key, value);
    }

    pub fn del(&self, key: &[u8]) -> bool {
        self.entries.lock().unwrap().remove(key).is_some()
    }
}
