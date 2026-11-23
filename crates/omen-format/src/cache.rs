//! Thread-safe bounded cache for formatters and formatting operations.
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::RwLock;

/// Bounded LRU-style cache to prevent unbounded memory growth while maximizing reuse.
pub struct BoundedCache<K, V> {
    capacity: usize,
    entries: RwLock<HashMap<K, V>>,
}

impl<K: Eq + Hash + Clone, V: Clone> BoundedCache<K, V> {
    pub fn new(capacity: usize) -> Self {
        Self {
            capacity,
            entries: RwLock::new(HashMap::new()),
        }
    }

    pub fn get(&self, key: &K) -> Option<V> {
        self.entries.read().ok()?.get(key).cloned()
    }

    pub fn insert(&self, key: K, value: V) {
        if let Ok(mut lock) = self.entries.write() {
            if lock.len() >= self.capacity {
                // Prune half when exceeding capacity
                let keys_to_remove: Vec<K> = lock.keys().take(self.capacity / 2).cloned().collect();
                for k in keys_to_remove {
                    lock.remove(&k);
                }
            }
            lock.insert(key, value);
        }
    }

    pub fn get_or_insert_with<F>(&self, key: K, f: F) -> V
    where
        F: FnOnce() -> V,
    {
        if let Some(existing) = self.get(&key) {
            return existing;
        }
        let created = f();
        self.insert(key, created.clone());
        created
    }
}

impl<K, V> Default for BoundedCache<K, V> {
    fn default() -> Self {
        Self {
            capacity: 256,
            entries: RwLock::new(HashMap::new()),
        }
    }
}
