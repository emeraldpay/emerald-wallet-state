use std::str::FromStr;
use std::sync::Arc;
use chrono::Utc;
use protobuf::Message;
use sled::{Batch, Db};
use uuid::Uuid;
use crate::access::allowance::Allowances;
use crate::access::pagination::PageResult;
use crate::errors::{InvalidValueError, StateError};
use crate::proto::balance::{Allowance};
use crate::{validate};

const PREFIX_KEY: &'static str = "allowance:";
const DEFAULT_TTL: u64 = 24 * 60 * 60 * 1000;
const MAX_TTL: u64 = 30 * DEFAULT_TTL;

pub struct AllowanceAccess {
    pub(crate) db: Arc<Db>,
}

impl AllowanceAccess {

    fn purge(&self) -> Result<usize, StateError> {
        let mut count = 0;
        let mut iter = self.db.scan_prefix(PREFIX_KEY);
        let mut batch = Batch::default();
        while let Some(entry) = iter.next() {
            if let Ok(entry) = &entry {
                let delete = if let Ok(allowance) = Allowance::parse_from_bytes(entry.1.as_ref()) {
                    allowance.ttl < Utc::now().naive_utc().timestamp_millis() as u64
                } else {
                    // always delete invalid entries
                    true
                };
                if delete {
                    count+=1;
                    batch.remove(entry.0.clone());
                }
            }
        }
        if count > 0 {
            let _ = self.db.apply_batch(batch);
        }
        Ok(count)
    }

}

impl Allowances for AllowanceAccess {
    fn add(&self, allowance: Allowance, ttl: Option<u64>) -> Result<(), StateError> {
        validate::check_ethereum_address(&allowance.token)
            .map_err(|_| InvalidValueError::Name("token".to_string()))?;
        validate::check_ethereum_address(&allowance.owner)
            .map_err(|_| InvalidValueError::Name("owner".to_string()))?;
        validate::check_ethereum_address(&allowance.spender)
            .map_err(|_| InvalidValueError::Name("spender".to_string()))?;
        let _ = Uuid::from_str(&allowance.wallet_id)
            .map_err(|_| InvalidValueError::Name("wallet_id".to_string()))?;

        let mut allowance = allowance.clone();
        allowance.ts = Utc::now().naive_utc().timestamp_millis() as u64;
        allowance.ttl = allowance.ts + ttl.or(Some(DEFAULT_TTL))
            .map(|v| if v > MAX_TTL { MAX_TTL } else { v })
            .unwrap();

        let key = format!("{}_{}_{}_{}_{}_{}", PREFIX_KEY, allowance.wallet_id, allowance.blockchain, allowance.token, allowance.owner, allowance.spender);

        self.db.insert(key.as_bytes(), allowance.write_to_bytes()?.as_slice())?;

        Ok(())
    }

    fn list(&self, wallet_id: Option<Uuid>) -> Result<PageResult<Allowance>, StateError> {
        let prefix = match wallet_id {
            None => PREFIX_KEY.to_string(),
            Some(wallet) => format!("{}_{}_", PREFIX_KEY, wallet.to_string())
        };
        let mut iter = self.db.scan_prefix(prefix);
        let mut result = vec![];
        let mut outdated = 0;
        while let Some(entry) = iter.next() {
            if let Ok(next) = entry {
                if let Ok(allowance) = Allowance::parse_from_bytes(next.1.as_ref()) {
                    if allowance.ttl < Utc::now().naive_utc().timestamp_millis() as u64 {
                        outdated += 1;
                        continue;
                    }
                    result.push(allowance);
                }
            }
        }

        if outdated > result.len() {
            let _ = self.purge();
        }

        Ok(PageResult {
            values: result,
            cursor: None
        })
    }

    fn remove(&self, wallet_id: Uuid, blockchain: Option<u32>, min_ts: Option<u64>) -> Result<usize, StateError> {
        let prefix = format!("{}_{}_", PREFIX_KEY, wallet_id.to_string());

        let mut iter = self.db.scan_prefix(prefix);
        let mut count = 0;
        let mut batch = Batch::default();
        while let Some(entry) = iter.next() {
            if let Ok(next) = entry {
                if let Ok(allowance) = Allowance::parse_from_bytes(next.1.as_ref()) {
                    let delete_by_blockchain = match blockchain {
                        None => true,
                        Some(blockchain) => allowance.blockchain == blockchain
                    };
                    let delete_by_ts = match min_ts {
                        None => true,
                        Some(ts) => allowance.ts < ts
                    };
                    if delete_by_blockchain && delete_by_ts {
                        count += 1;
                        batch.remove(next.0.clone());
                    }
                }
            }
        }

        if count > 0 {
            let _ = self.db.apply_batch(batch);
        }
        Ok(count)
    }

}

#[cfg(test)]
mod tests {
    use std::str::FromStr;
    use std::thread;
    use std::time::Duration;
    use chrono::Utc;
    use tempdir::TempDir;
    use uuid::Uuid;
    use crate::access::allowance::Allowances;
    use crate::proto::balance::Allowance;
    use crate::storage::sled_access::SledStorage;

    #[test]
    fn add_and_list() {
        let tmp_dir = TempDir::new("test-allowance").unwrap();
        let access = SledStorage::open(tmp_dir.path().to_path_buf()).unwrap();
        let store = access.get_allowance();

        let mut item = Allowance::new();
        item.wallet_id = "5e0e8fb5-9ffb-4b18-b79a-b732d19576f3".to_string();
        item.blockchain = 100;
        item.token = "0xdAC17F958D2ee523a2206206994597C13D831ec7".to_string();
        item.owner = "0x9696f59E4d72E237BE84fFD425DCaD154Bf96976".to_string();
        item.spender = "0x65A0947BA5175359Bb457D3b34491eDf4cBF7997".to_string();
        item.amount = "10000000".to_string();

        let added = store.add(item.clone(), None);
        assert!(added.is_ok());

        let all = store.list(None);
        assert_eq!(all.is_ok(), true);
        let all = all.unwrap();
        assert_eq!(all.values.len(), 1);
        assert_eq!(all.values[0].blockchain, item.blockchain);
        assert_eq!(all.values[0].token, item.token);
        assert_eq!(all.values[0].owner, item.owner);
        assert_eq!(all.values[0].spender, item.spender);
        assert_eq!(all.values[0].amount, item.amount);

        let all_by_wallet = store.list(Some(Uuid::from_str("5e0e8fb5-9ffb-4b18-b79a-b732d19576f3").unwrap()));
        assert_eq!(all_by_wallet.is_ok(), true);
        assert_eq!(all_by_wallet.unwrap().values.len(), 1);
    }

    #[test]
    fn add_and_list_by_wallet() {
        let tmp_dir = TempDir::new("test-allowance").unwrap();
        let access = SledStorage::open(tmp_dir.path().to_path_buf()).unwrap();
        let store = access.get_allowance();

        let mut item = Allowance::new();
        item.wallet_id = "5e0e8fb5-9ffb-4b18-b79a-b732d19576f3".to_string();
        item.blockchain = 100;
        item.token = "0xdAC17F958D2ee523a2206206994597C13D831ec7".to_string();
        item.owner = "0x9696f59E4d72E237BE84fFD425DCaD154Bf96976".to_string();
        item.spender = "0x65A0947BA5175359Bb457D3b34491eDf4cBF7997".to_string();
        item.amount = "10000000".to_string();

        let added = store.add(item.clone(), None);
        assert!(added.is_ok());

        let all_by_wallet = store.list(Some(Uuid::from_str("5e0e8fb5-9ffb-4b18-b79a-b732d19576f3").unwrap()));
        assert_eq!(all_by_wallet.is_ok(), true);
        assert_eq!(all_by_wallet.unwrap().values.len(), 1);
    }

    #[test]
    fn add_and_remove_by_wallet() {
        let tmp_dir = TempDir::new("test-allowance").unwrap();
        let access = SledStorage::open(tmp_dir.path().to_path_buf()).unwrap();
        let store = access.get_allowance();

        let mut item_1 = Allowance::new();
        item_1.wallet_id = "5e0e8fb5-9ffb-4b18-b79a-b732d19576f3".to_string();
        item_1.blockchain = 100;
        item_1.token = "0xdAC17F958D2ee523a2206206994597C13D831ec7".to_string();
        item_1.owner = "0x9696f59E4d72E237BE84fFD425DCaD154Bf96976".to_string();
        item_1.spender = "0x65A0947BA5175359Bb457D3b34491eDf4cBF7997".to_string();
        item_1.amount = "10000000".to_string();
        let _ = store.add(item_1.clone(), None).unwrap();

        let mut item_2 = Allowance::new();
        item_2.wallet_id = "5e0e8fb5-9ffb-4b18-b79a-b732d19576f3".to_string();
        item_2.blockchain = 101;
        item_2.token = "0xdAC17F958D2ee523a2206206994597C13D831ec7".to_string();
        item_2.owner = "0x9696f59E4d72E237BE84fFD425DCaD154Bf96976".to_string();
        item_2.spender = "0x65A0947BA5175359Bb457D3b34491eDf4cBF7997".to_string();
        item_2.amount = "9000000".to_string();

        let _ = store.add(item_2.clone(), None).unwrap();

        let removed = store.remove(Uuid::from_str("5e0e8fb5-9ffb-4b18-b79a-b732d19576f3").unwrap(), None, None);
        assert_eq!(removed.is_ok(), true);
        assert_eq!(removed.unwrap(), 2);

        let all_by_wallet = store.list(Some(Uuid::from_str("5e0e8fb5-9ffb-4b18-b79a-b732d19576f3").unwrap())).unwrap();
        assert_eq!(all_by_wallet.values.len(), 0);
    }

    #[test]
    fn add_and_remove_by_blockchain() {
        let tmp_dir = TempDir::new("test-allowance").unwrap();
        let access = SledStorage::open(tmp_dir.path().to_path_buf()).unwrap();
        let store = access.get_allowance();

        let mut item_1 = Allowance::new();
        item_1.wallet_id = "5e0e8fb5-9ffb-4b18-b79a-b732d19576f3".to_string();
        item_1.blockchain = 100;
        item_1.token = "0xdAC17F958D2ee523a2206206994597C13D831ec7".to_string();
        item_1.owner = "0x9696f59E4d72E237BE84fFD425DCaD154Bf96976".to_string();
        item_1.spender = "0x65A0947BA5175359Bb457D3b34491eDf4cBF7997".to_string();
        item_1.amount = "10000000".to_string();
        let _ = store.add(item_1.clone(), None).unwrap();

        let mut item_2 = Allowance::new();
        item_2.wallet_id = "5e0e8fb5-9ffb-4b18-b79a-b732d19576f3".to_string();
        item_2.blockchain = 101;
        item_2.token = "0xdAC17F958D2ee523a2206206994597C13D831ec7".to_string();
        item_2.owner = "0x9696f59E4d72E237BE84fFD425DCaD154Bf96976".to_string();
        item_2.spender = "0x65A0947BA5175359Bb457D3b34491eDf4cBF7997".to_string();
        item_2.amount = "9000000".to_string();

        let _ = store.add(item_2.clone(), None).unwrap();

        let removed = store.remove(Uuid::from_str("5e0e8fb5-9ffb-4b18-b79a-b732d19576f3").unwrap(), Some(101), None);
        assert_eq!(removed.is_ok(), true);
        assert_eq!(removed.unwrap(), 1);

        let all_by_wallet = store.list(Some(Uuid::from_str("5e0e8fb5-9ffb-4b18-b79a-b732d19576f3").unwrap())).unwrap();
        assert_eq!(all_by_wallet.values.len(), 1);
        assert_eq!(all_by_wallet.values[0].amount, item_1.amount);
    }

    #[test]
    fn add_and_remove_by_ts() {
        let tmp_dir = TempDir::new("test-allowance").unwrap();
        let access = SledStorage::open(tmp_dir.path().to_path_buf()).unwrap();
        let store = access.get_allowance();

        let ts_0 = Utc::now().naive_utc().timestamp_millis() as u64;

        let mut item_1 = Allowance::new();
        item_1.wallet_id = "5e0e8fb5-9ffb-4b18-b79a-b732d19576f3".to_string();
        item_1.blockchain = 100;
        item_1.token = "0xdAC17F958D2ee523a2206206994597C13D831ec7".to_string();
        item_1.owner = "0x9696f59E4d72E237BE84fFD425DCaD154Bf96976".to_string();
        item_1.spender = "0x65A0947BA5175359Bb457D3b34491eDf4cBF7997".to_string();
        item_1.amount = "10000000".to_string();
        let _ = store.add(item_1.clone(), None).unwrap();

        thread::sleep(Duration::from_millis(50));
        let ts_1 = Utc::now().naive_utc().timestamp_millis() as u64;

        let mut item_2 = Allowance::new();
        item_2.wallet_id = "5e0e8fb5-9ffb-4b18-b79a-b732d19576f3".to_string();
        item_2.blockchain = 101;
        item_2.token = "0xdAC17F958D2ee523a2206206994597C13D831ec7".to_string();
        item_2.owner = "0x9696f59E4d72E237BE84fFD425DCaD154Bf96976".to_string();
        item_2.spender = "0x65A0947BA5175359Bb457D3b34491eDf4cBF7997".to_string();
        item_2.amount = "9000000".to_string();

        let _ = store.add(item_2.clone(), None).unwrap();

        thread::sleep(Duration::from_millis(50));
        let ts_2 = Utc::now().naive_utc().timestamp_millis() as u64;


        let removed = store.remove(Uuid::from_str("5e0e8fb5-9ffb-4b18-b79a-b732d19576f3").unwrap(), None, Some(ts_0)).unwrap();
        assert_eq!(removed, 0);

        let all_by_wallet = store.list(Some(Uuid::from_str("5e0e8fb5-9ffb-4b18-b79a-b732d19576f3").unwrap())).unwrap();
        assert_eq!(all_by_wallet.values.len(), 2);

        let removed = store.remove(Uuid::from_str("5e0e8fb5-9ffb-4b18-b79a-b732d19576f3").unwrap(), None, Some(ts_1)).unwrap();
        assert_eq!(removed, 1);

        let all_by_wallet = store.list(Some(Uuid::from_str("5e0e8fb5-9ffb-4b18-b79a-b732d19576f3").unwrap())).unwrap();
        assert_eq!(all_by_wallet.values.len(), 1);
        assert_eq!(all_by_wallet.values[0].amount, item_2.amount);

        let removed = store.remove(Uuid::from_str("5e0e8fb5-9ffb-4b18-b79a-b732d19576f3").unwrap(), None, Some(ts_2)).unwrap();
        assert_eq!(removed, 1);
        let all_by_wallet = store.list(Some(Uuid::from_str("5e0e8fb5-9ffb-4b18-b79a-b732d19576f3").unwrap())).unwrap();
        assert_eq!(all_by_wallet.values.len(), 0);
    }
}