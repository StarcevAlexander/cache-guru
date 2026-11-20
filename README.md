# cache-guru

**Go-style TTL cache for Rust** — a thread-safe in-memory cache with expiry, FIFO eviction when capacity is set and hit/miss statistics. Zero dependencies.

Formerly published on crates.io as `cacheguru`.

For developers coming from Go's `go-cache` / `ristretto` or Node's `node-cache` who want a small, obvious cache without pulling a framework.

## Usage

```rust
use cache_guru::Cache;
use std::time::Duration;

fn main() {
    // TTL cache: entries expire after 30 seconds
    let cache = Cache::new(Duration::from_secs(30));
    cache.set("token", "abc123");
    assert_eq!(cache.get("token"), Some("abc123".to_string()));

    // Per-entry TTL override
    cache.set_with_ttl("short", 1, Duration::from_millis(100));

    // Eternal cache with FIFO eviction above capacity
    let eternal = Cache::eternal().with_max_entries(100);

    let stats = cache.stats();
    println!("hits: {}, misses: {}", stats.hits, stats.misses);
}
```

## API

- `Cache::new(ttl)` — all entries expire after `ttl`
- `Cache::eternal()` — entries never expire on their own
- `.with_max_entries(n)` — FIFO eviction when capacity is exceeded
- `set` / `set_with_ttl` / `get` / `remove` / `contains` / `clear`
- `purge_expired()` — bulk-remove expired entries, returns the count
- `stats()` — `hits` / `misses` / `entries`

Thread-safe: every method takes `&self`, suitable for sharing behind `Arc`.

## License

Licensed under either of MIT or Apache-2.0, at your option.

Name is a tribute to Go's [go-cache](https://github.com/patrickmn/go-cache) and [ristretto](https://github.com/dgraph-io/ristretto); this crate is an independent implementation for Rust with zero dependencies.
