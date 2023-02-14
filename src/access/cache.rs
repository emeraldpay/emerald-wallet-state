use chrono::{DateTime, Utc};
use crate::errors::StateError;
use crate::proto::cache::{Cache as proto_Cache};

#[derive(Debug, Clone, PartialEq)]
pub struct CacheEntry {
    pub id: String,
    pub ts: DateTime<Utc>,
    pub ttl: DateTime<Utc>,
    pub value: String,
}

///
/// Generic cache
pub trait Cache {

    ///
    /// Put a `value` encoded as a string into the cache. `ttl_seconds` defined for how long it can be keptin cache.
    /// Note that the value may live longer or shorter than the proposed `ttl_seconds`
    fn put(&mut self, id: String, value: String, ttl_seconds: Option<u64>) -> Result<(), StateError>;

    ///
    /// Get value from cache. Returns `None` if nothing found for the specified `id`
    fn get(&self, id: String) -> Result<Option<String>, StateError>;

    ///
    /// Remove value from cache
    fn evict(&mut self, id: String) -> Result<(), StateError>;

    ///
    /// Remove all values in cache that has an expired ttl
    fn purge(&mut self) -> Result<usize, StateError>;

}

impl Into<proto_Cache> for CacheEntry {
    fn into(self) -> proto_Cache {
        let mut result = proto_Cache::new();
        result.set_value(self.value);
        result.set_id(self.id);
        result.set_ts(self.ts.timestamp_millis() as u64);
        result.set_ttl(self.ttl.timestamp_millis() as u64);
        result
    }
}
