use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

// std Mutex is fine here because the lock is never held across an .await.
#[derive(Clone, Default)]
pub struct Db {
    entries: Arc<Mutex<HashMap<Vec<u8>, Entry>>>,
}

struct Entry {
    value: Vec<u8>,
    expires_at: Option<Instant>,
}

#[derive(Debug, PartialEq)]
pub enum Ttl {
    Missing,
    Persistent,
    Remaining(Duration),
}

impl Db {
    pub fn get(&self, key: &[u8], now: Instant) -> Option<Vec<u8>> {
        let mut entries = self.entries.lock().unwrap();
        live(&mut entries, key, now).map(|entry| entry.value.clone())
    }

    pub fn set(&self, key: Vec<u8>, value: Vec<u8>) {
        let entry = Entry {
            value,
            expires_at: None,
        };
        self.entries.lock().unwrap().insert(key, entry);
    }

    pub fn del(&self, key: &[u8], now: Instant) -> bool {
        let removed = self.entries.lock().unwrap().remove(key);
        removed.is_some_and(|entry| !entry.is_expired(now))
    }

    pub fn expire(&self, key: &[u8], at: Instant, now: Instant) -> bool {
        let mut entries = self.entries.lock().unwrap();
        live(&mut entries, key, now)
            .map(|entry| entry.expires_at = Some(at))
            .is_some()
    }

    pub fn ttl(&self, key: &[u8], now: Instant) -> Ttl {
        let mut entries = self.entries.lock().unwrap();
        match live(&mut entries, key, now) {
            None => Ttl::Missing,
            Some(Entry {
                expires_at: None, ..
            }) => Ttl::Persistent,
            Some(Entry {
                expires_at: Some(at),
                ..
            }) => Ttl::Remaining(*at - now),
        }
    }
}

impl Entry {
    fn is_expired(&self, now: Instant) -> bool {
        self.expires_at.is_some_and(|at| at <= now)
    }
}

/// Looks up `key`, deleting it on the way if it has expired.
fn live<'a>(
    entries: &'a mut HashMap<Vec<u8>, Entry>,
    key: &[u8],
    now: Instant,
) -> Option<&'a mut Entry> {
    // Two lookups because the borrow checker won't let us remove while `get` holds a borrow.
    if entries.get(key)?.is_expired(now) {
        entries.remove(key);
        return None;
    }
    entries.get_mut(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expired_keys_are_gone() {
        let db = Db::default();
        let now = Instant::now();
        db.set(b"k".to_vec(), b"v".to_vec());
        assert!(db.expire(b"k", now + Duration::from_secs(10), now));

        let later = now + Duration::from_secs(9);
        assert_eq!(db.get(b"k", later), Some(b"v".to_vec()));
        assert_eq!(db.ttl(b"k", later), Ttl::Remaining(Duration::from_secs(1)));

        let expired = now + Duration::from_secs(10);
        assert_eq!(db.get(b"k", expired), None);
        assert_eq!(db.ttl(b"k", expired), Ttl::Missing);
        assert!(!db.del(b"k", expired));
    }

    #[test]
    fn set_clears_expiry() {
        let db = Db::default();
        let now = Instant::now();
        db.set(b"k".to_vec(), b"v".to_vec());
        db.expire(b"k", now + Duration::from_secs(10), now);
        db.set(b"k".to_vec(), b"v2".to_vec());
        assert_eq!(db.ttl(b"k", now), Ttl::Persistent);
    }

    #[test]
    fn expire_on_missing_key() {
        let db = Db::default();
        let now = Instant::now();
        assert!(!db.expire(b"nope", now + Duration::from_secs(1), now));
    }
}
