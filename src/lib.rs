use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

pub struct Stats {
    pub hits: u64,
    pub misses: u64,
    pub entries: usize,
}

struct Entry<V> {
    value: V,
    expires_at: Option<Instant>,
    inserted_at: Instant,
}

pub struct Cache<V: Clone> {
    map: Mutex<HashMap<String, Entry<V>>>,
    ttl: Option<Duration>,
    max_entries: Option<usize>,
    hits: Mutex<u64>,
    misses: Mutex<u64>,
}

impl<V: Clone> Cache<V> {
    pub fn new(ttl: Duration) -> Self {
        Self {
            map: Mutex::new(HashMap::new()),
            ttl: Some(ttl),
            max_entries: None,
            hits: Mutex::new(0),
            misses: Mutex::new(0),
        }
    }

    pub fn eternal() -> Self {
        Self {
            map: Mutex::new(HashMap::new()),
            ttl: None,
            max_entries: None,
            hits: Mutex::new(0),
            misses: Mutex::new(0),
        }
    }

    pub fn with_max_entries(mut self, max: usize) -> Self {
        self.max_entries = Some(max.max(1));
        self
    }

    pub fn set(&self, key: impl Into<String>, value: V) {
        self.insert(key.into(), value, self.ttl);
    }

    pub fn set_with_ttl(&self, key: impl Into<String>, value: V, ttl: Duration) {
        self.insert(key.into(), value, Some(ttl));
    }

    fn insert(&self, key: String, value: V, ttl: Option<Duration>) {
        let expires_at = ttl.map(|ttl| Instant::now() + ttl);
        let mut map = self.map.lock().expect("cache lock poisoned");
        let evict_needed = self
            .max_entries
            .is_some_and(|max| !map.contains_key(&key) && map.len() >= max);
        if evict_needed
            && let Some(oldest) = map
                .iter()
                .min_by_key(|(_, e)| e.inserted_at)
                .map(|(k, _)| k.clone())
        {
            map.remove(&oldest);
        }
        map.insert(
            key,
            Entry {
                value,
                expires_at,
                inserted_at: Instant::now(),
            },
        );
    }

    pub fn get(&self, key: &str) -> Option<V> {
        let mut map = self.map.lock().expect("cache lock poisoned");
        let now = Instant::now();
        if let Some(entry) = map.get(key) {
            let expired = entry.expires_at.map(|at| at <= now).unwrap_or(false);
            if !expired {
                *self.hits.lock().expect("hits lock poisoned") += 1;
                return Some(entry.value.clone());
            }
            map.remove(key);
            *self.misses.lock().expect("hits lock poisoned") += 1;
            return None;
        }
        *self.misses.lock().expect("hits lock poisoned") += 1;
        None
    }

    pub fn remove(&self, key: &str) -> Option<V> {
        let mut map = self.map.lock().expect("cache lock poisoned");
        map.remove(key).map(|e| e.value)
    }

    pub fn contains(&self, key: &str) -> bool {
        let map = self.map.lock().expect("cache lock poisoned");
        map.contains_key(key)
    }

    pub fn purge_expired(&self) -> usize {
        let mut map = self.map.lock().expect("cache lock poisoned");
        let now = Instant::now();
        let expired: Vec<String> = map
            .iter()
            .filter(|(_, e)| e.expires_at.map(|at| at <= now).unwrap_or(false))
            .map(|(k, _)| k.clone())
            .collect();
        for key in &expired {
            map.remove(key);
        }
        expired.len()
    }

    pub fn len(&self) -> usize {
        self.map.lock().expect("cache lock poisoned").len()
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    pub fn clear(&self) {
        self.map.lock().expect("cache lock poisoned").clear();
    }

    pub fn stats(&self) -> Stats {
        Stats {
            hits: *self.hits.lock().expect("hits lock poisoned"),
            misses: *self.misses.lock().expect("hits lock poisoned"),
            entries: self.len(),
        }
    }
}

impl<V: Clone> Default for Cache<V> {
    fn default() -> Self {
        Self::eternal()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn set_get_roundtrip() {
        let cache = Cache::eternal();
        cache.set("a", 1);
        assert_eq!(cache.get("a"), Some(1));
        assert_eq!(cache.get("b"), None);
    }

    #[test]
    fn ttl_expires_entries() {
        let cache = Cache::new(Duration::from_millis(30));
        cache.set("k", "v");
        assert_eq!(cache.get("k"), Some("v"));
        thread::sleep(Duration::from_millis(60));
        assert_eq!(cache.get("k"), None);
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn eternal_never_expires() {
        let cache = Cache::eternal();
        cache.set("k", 5);
        thread::sleep(Duration::from_millis(40));
        assert_eq!(cache.get("k"), Some(5));
    }

    #[test]
    fn max_entries_evicts_oldest() {
        let cache = Cache::eternal().with_max_entries(2);
        cache.set("a", 1);
        thread::sleep(Duration::from_millis(5));
        cache.set("b", 2);
        thread::sleep(Duration::from_millis(5));
        cache.set("c", 3);
        assert!(cache.get("a").is_none());
        assert!(cache.get("b").is_some());
        assert!(cache.get("c").is_some());
    }

    #[test]
    fn purge_removes_expired_only() {
        let cache = Cache::new(Duration::from_millis(20));
        cache.set("short", 1);
        cache.set_with_ttl("long", 2, Duration::from_secs(60));
        thread::sleep(Duration::from_millis(40));
        let purged = cache.purge_expired();
        assert_eq!(purged, 1);
        assert_eq!(cache.len(), 1);
        assert_eq!(cache.get("long"), Some(2));
    }

    #[test]
    fn stats_count_hits_and_misses() {
        let cache = Cache::eternal();
        cache.set("a", 1);
        let _ = cache.get("a");
        let _ = cache.get("a");
        let _ = cache.get("zzz");
        let stats = cache.stats();
        assert_eq!(stats.hits, 2);
        assert_eq!(stats.misses, 1);
        assert_eq!(stats.entries, 1);
    }

    #[test]
    fn remove_and_clear() {
        let cache = Cache::eternal();
        cache.set("a", 1);
        assert_eq!(cache.remove("a"), Some(1));
        assert!(cache.get("a").is_none());
        cache.set("b", 2);
        cache.clear();
        assert!(cache.is_empty());
    }
}
