use std::sync::Arc;
use chrono::{Duration, TimeZone, Utc};
use protobuf::Message;
use sled::{Batch, Db};
use crate::access::cache::{Cache, CacheEntry};
use crate::proto::cache::{Cache as proto_Cache};
use crate::errors::StateError;

const PREFIX_KEY: &'static str = "cache:";

// 1 week by default
const DEFAULT_TTL_SECOND: u64 = 60 * 60 * 24 * 7;
// 1 month
const MAX_TTL_SECOND: u64 = 60 * 60 * 24 * 30;

const PURGE_KEY: &str = "_purge";
// purge cache every 1 hour
const PURGE_EVERY_SECONDS: i64 = 60 * 60;

pub struct CacheAccess {
    pub(crate) db: Arc<Db>,
}

impl CacheAccess {

    fn get_key(id: &String) -> String {
        format!("{}{}", PREFIX_KEY, id.to_string())
    }

    fn should_purge(&self) -> bool {
        let last_purge = self.get(PURGE_KEY.to_string())
            .or::<StateError>(Ok(None))
            .unwrap()
            .or(Some("0".to_string()))
            .map(|v| v.parse::<i64>())
            .unwrap()
            .or::<StateError>(Ok(0i64))
            .unwrap();

        Utc.timestamp_millis(last_purge).lt(
            &Utc::now()
                .checked_sub_signed(Duration::seconds(PURGE_EVERY_SECONDS))
                .unwrap()
        )
    }

    fn mark_purged(&mut self) {
        let _ = self.put(
            PURGE_KEY.to_string(),
            Utc::now().timestamp_millis().to_string(),
            Some(MAX_TTL_SECOND)
        );
    }

}

impl Cache for CacheAccess {

    fn put(&mut self, id: String, value: String, ttl_seconds: Option<u64>) -> Result<(), StateError> {
        let duration = ttl_seconds.or(Some(DEFAULT_TTL_SECOND))
            .map(|v| if v > MAX_TTL_SECOND { MAX_TTL_SECOND } else {v})
            .map(|v| Duration::seconds(v as i64))
            .unwrap();
        let entry = CacheEntry {
            id: id.clone(),
            value,
            ts: Utc::now(),
            ttl: Utc::now()
                .checked_add_signed(duration)
                .unwrap()
        };
        let proto: proto_Cache = entry.into();
        if let Ok(bytes) = proto.write_to_bytes() {
            self.db.insert(CacheAccess::get_key(&id).as_bytes(), bytes)?;
        }
        if self.should_purge() {
            let _ = self.purge();
        }
        Ok(())
    }

    fn get(&self, id: String) -> Result<Option<String>, StateError> {
        let key = CacheAccess::get_key(&id);
        if let Some(base) = self.db.get(&key)? {
            let proto = proto_Cache::parse_from_bytes(base.as_ref())?;
            Ok(Some(proto.value))
        } else {
            Ok(None)
        }
    }

    fn evict(&mut self, id: String) -> Result<(), StateError> {
        self.db.remove(CacheAccess::get_key(&id).as_bytes())
            .map(|_| ())
            .map_err(StateError::from)
    }

    fn purge(&mut self) -> Result<usize, StateError> {
        let mut iter = self.db.scan_prefix(PREFIX_KEY);
        let mut done = false;
        let mut count = 0;
        let mut batch = Batch::default();
        while !done {
            let next = iter.next();
            match next {
                Some(entry) => {
                    if let Ok(entry) = entry {
                        let delete = if let Ok(proto) = proto_Cache::parse_from_bytes(entry.1.as_ref()) {
                            Utc.timestamp_millis(proto.get_ttl() as i64)
                                .lt(&Utc::now())
                        } else {
                            // always delete corrupted values
                            true
                        };
                        if delete {
                            count+=1;
                            batch.remove(entry.0);
                        }
                    }
                },
                None => done = true
            }
        }
        if count > 0 {
            let _ = self.db.apply_batch(batch);
        }
        self.mark_purged();
        Ok(count)
    }
}


#[cfg(test)]
mod tests {
    use tempdir::TempDir;
    use crate::access::cache::Cache;
    use crate::storage::sled_access::SledStorage;

    #[test]
    fn get_nothing_exist() {
        let tmp_dir = TempDir::new("cache").unwrap();
        let access = SledStorage::open(tmp_dir.path().to_path_buf()).unwrap();
        let cache = access.get_cache();

        let act = cache.get("test".to_string());
        assert!(act.is_ok());
        let act = act.unwrap();
        assert!(act.is_none());
    }

    #[test]
    fn put_and_get_value() {
        let tmp_dir = TempDir::new("cache").unwrap();
        let access = SledStorage::open(tmp_dir.path().to_path_buf()).unwrap();
        let mut cache = access.get_cache();

        let put = cache.put("test".to_string(), "hello world!".to_string(), None);
        assert!(put.is_ok());

        let act = cache.get("test".to_string());
        assert!(act.is_ok());
        let act = act.unwrap();
        assert!(act.is_some());
        let act = act.unwrap();
        assert_eq!(act, "hello world!")
    }

    #[test]
    fn put_and_evict_value() {
        let tmp_dir = TempDir::new("cache").unwrap();
        let access = SledStorage::open(tmp_dir.path().to_path_buf()).unwrap();
        let mut cache = access.get_cache();

        let put = cache.put("test".to_string(), "hello world!".to_string(), None);
        assert!(put.is_ok());

        let evict = cache.evict("test".to_string());
        assert!(evict.is_ok());

        let act = cache.get("test".to_string());
        assert!(act.is_ok());
        let act = act.unwrap();
        assert!(act.is_none());
    }

    #[test]
    fn purge_doesnt_delete_fresh_values() {
        let tmp_dir = TempDir::new("cache").unwrap();
        let access = SledStorage::open(tmp_dir.path().to_path_buf()).unwrap();
        let mut cache = access.get_cache();

        let put = cache.put("test".to_string(), "hello world!".to_string(), None);
        assert!(put.is_ok());

        let evict = cache.purge();
        assert!(evict.is_ok());
        assert_eq!(0, evict.unwrap());

        let act = cache.get("test".to_string());
        assert!(act.is_ok());
        let act = act.unwrap();
        assert!(act.is_some());
    }

    #[test]
    fn purge_deletes_expired_values() {
        let tmp_dir = TempDir::new("cache").unwrap();
        let access = SledStorage::open(tmp_dir.path().to_path_buf()).unwrap();
        let mut cache = access.get_cache();

        let put = cache.put("test".to_string(), "hello world!".to_string(), Some(1));
        assert!(put.is_ok());

        std::thread::sleep(core::time::Duration::from_secs(2));

        let evict = cache.purge();
        assert!(evict.is_ok());
        assert_eq!(1, evict.unwrap());

        let act = cache.get("test".to_string());
        assert!(act.is_ok());
        let act = act.unwrap();
        assert!(act.is_none());
    }
}