use std::collections::HashMap;
use std::time::{Duration, Instant};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Default)]
pub struct Store {
    data: HashMap<String, String>,
    expirations: HashMap<String, Instant>,
}

fn now_unix_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
}

impl Store {
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
            expirations: HashMap::new(),
        }
    }

    pub fn expire_at(&mut self, key: &str, unix_timestamp_ms: u128) -> bool {
        if !self.exists(key) {
            return false;
        }

        let now_ms = now_unix_ms();

        if unix_timestamp_ms <= now_ms {
            self.del(key);
            return true;
        }

        let duration = Duration::from_millis((unix_timestamp_ms - now_ms) as u64);
        let expiration = Instant::now() + duration;

        self.expirations.insert(key.to_string(), expiration);

        true
    }

    pub fn expiration_unix_ms_from_seconds(seconds: u64) -> u128 {
        now_unix_ms() + Duration::from_secs(seconds).as_millis()
    }

    pub fn set_ex(&mut self, key: String, value: String, seconds: u64) {
        self.data.insert(key.clone(), value);

        let expiration = Instant::now() + Duration::from_secs(seconds);

        self.expirations.insert(key, expiration);
    }

    pub fn set(&mut self, key: String, value: String) {
        self.expirations.remove(&key);
        self.data.insert(key, value);
    }

    pub fn get(&self, key: &str) -> Option<&String> {
        // Read-only: a logically-expired key reads as absent, but is left for
        // the background cleaner to remove. Keeping this `&self` lets the server
        // serve concurrent reads under a shared lock.
        if let Some(expiration) = self.expirations.get(key) {
            if Instant::now() >= *expiration {
                return None;
            }
        }
        self.data.get(key)
    }

    pub fn del(&mut self, key: &str) -> bool {
        self.expirations.remove(key);
        self.data.remove(key).is_some()
    }

    pub fn exists(&self, key: &str) -> bool {
        self.get(key).is_some()
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn expire(&mut self, key: &str, seconds: u64) -> bool {
        if !self.exists(key) {
            return false;
        }

        let expiration = Instant::now() + Duration::from_secs(seconds);

        self.expirations.insert(key.to_string(), expiration);

        true
    }

    pub fn ttl(&self, key: &str) -> i64 {
        if !self.exists(key) {
            return -2;
        }

        match self.expirations.get(key) {
            Some(expiration) => {
                let now = Instant::now();

                if now >= *expiration {
                    // Logically expired; reported as missing, removed later by
                    // the background cleaner.
                    -2
                } else {
                    expiration.duration_since(now).as_secs() as i64
                }
            }
            None => -1,
        }
    }

    pub fn cleanup_expired_keys(&mut self) -> usize {
        let now = Instant::now();

        let expired_keys: Vec<String> = self
            .expirations
            .iter()
            .filter(|(_, expiration)| now >= **expiration)
            .map(|(key, _)| key.clone())
            .collect();

        let count = expired_keys.len();

        for key in expired_keys {
            self.data.remove(&key);
            self.expirations.remove(&key);
        }

        count
    }

    pub fn entries(&mut self) -> Vec<(String, String)> {
        self.cleanup_expired_keys();

        self.data
            .iter()
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect()
    }

    pub fn key_count(&mut self) -> usize {
        self.cleanup_expired_keys();
        self.data.len()
    }

    pub fn flush_all(&mut self) {
        self.data.clear();
        self.expirations.clear();
    }
}
