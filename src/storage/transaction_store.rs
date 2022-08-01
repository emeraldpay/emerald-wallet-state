use std::collections::HashSet;
use std::ops::{Bound, Deref};
use std::str::FromStr;
use std::sync::Arc;
use chrono::{DateTime, TimeZone, Utc};
use protobuf::{Message, ProtobufEnum};
use sled::{Batch, Db};
use uuid::Uuid;
use crate::access::transactions::{Filter, RemoteCursor, Transactions, WalletRef};
use crate::access::pagination::{PageResult, PageQuery};
use crate::errors::{StateError,InvalidValueError};
use crate::proto::transactions::{Transaction as proto_Transaction, Cursor as proto_Cursor, TransactionMeta as proto_TransactionMeta};
use crate::storage::indexing::{IndexedValue, QueryRanges, IndexConvert, IndexEncoding, Indexing};

///
/// # Storage:
///
/// - `tx:<UUID>` to store transaction data
/// - `idx:tx:<INDEX>` for indexes, where the value is a UUID to reference the Transactions Data
///
/// # Indexes:
///
/// - `1/<TIMESTAMP>`
/// - `2/<WALLET_ID>/<TIMESTAMP>`
///
///

const PREFIX_KEY: &'static str = "tx";
const PREFIX_KEY_META: &'static str = "txmeta";
const PREFIX_IDX: &'static str = "idx:tx";
const PREFIX_CURSOR: &'static str = "addr_cursor";

enum IndexType {
    // `<WALLET_ID>/<TIMESTAMP>`
    ByWallet(Uuid, u64),
    // `/<TIMESTAMP>`
    Everything(u64),
}

impl IndexType {
    fn get_prefix(&self) -> usize {
        match self {
            IndexType::Everything(_) => 1,
            IndexType::ByWallet(_, _) => 2,
        }
    }
}

impl IndexEncoding for IndexType {
    fn get_index_key(&self) -> String {
        match self {
            IndexType::ByWallet(wallet_id, ts) => format!("{}:{:}/{:}/{:}", PREFIX_IDX, self.get_prefix(), wallet_id, IndexConvert::get_desc_timestamp(*ts)),
            IndexType::Everything(ts) => format!("{}:{:}/{:}", PREFIX_IDX, self.get_prefix(), IndexConvert::get_desc_timestamp(*ts))
        }
    }
}

impl IndexedValue<IndexType> for proto_Transaction {

    fn get_index(&self) -> Vec<IndexType> {
        let mut keys: Vec<IndexType> = Vec::new();
        let blockchain: u32 = self.get_blockchain().value() as u32;

        let timestamps: Vec<u64> = vec![
            self.since_timestamp,
            self.confirm_timestamp,
        ]
            .iter()
            .filter(|ts| **ts > 0u64)
            .map(|ts| *ts)
            .collect();

        for ts in &timestamps {
            keys.push(IndexType::Everything(*ts))
        }

        for change in self.get_changes() {
            if let Ok(wallet_id) = Uuid::from_str(change.get_wallet_id()) {
                for ts in &timestamps {
                    keys.push(IndexType::ByWallet(wallet_id, *ts));
                }
            }
        }

        keys
    }
}


impl QueryRanges for Filter {
    fn get_index_bounds(&self) -> (Bound<String>, Bound<String>) {
        // TODO use wallet index if a wallet specified in filter
        let now = IndexType::Everything(Utc::now().naive_utc().timestamp_millis() as u64)
            .get_index_key();
        let start = IndexType::Everything(0u64).get_index_key();
        (Bound::Included(now), Bound::Included(start))
    }
}

pub struct TransactionsAccess {
    pub(crate) db: Arc<Db>,
}

impl TransactionsAccess {
    fn get_key<S: Into<String>>(blockchain: u32, txid: S) -> String {
        format!("{}:{}/{}", PREFIX_KEY, blockchain, txid.into())
    }
    fn get_key_meta<S: Into<String>>(blockchain: u32, txid: S) -> String {
        format!("{}:{}/{}", PREFIX_KEY_META, blockchain, txid.into())
    }

    fn get_tx_by_key(&self, key: String) -> Option<proto_Transaction> {
        match self.db.get(key) {
            Ok(data) => {
                match data {
                    Some(b) => proto_Transaction::parse_from_bytes(b.deref()).ok(),
                    None => None
                }
            }
            Err(_) => None
        }
    }
}

impl Transactions for TransactionsAccess {

    fn query(&self, filter: Filter, page: PageQuery) -> Result<PageResult<proto_Transaction>, StateError> {
        let bounds = filter.get_index_bounds();
        let mut processed = HashSet::new();
        let mut iter = self.db.range(bounds);
        let mut done = false;

        let mut txes = Vec::new();

        while !done {
            let next = iter.next();
            match next {
                Some(x) => match x {
                    Ok(v) => {
                        let txkey = v.1.to_vec();
                        let txkey = String::from_utf8(txkey).unwrap();
                        let unprocessed = processed.insert(txkey.clone());
                        if unprocessed {
                            if let Some(tx) = self.get_tx_by_key(txkey) {
                                if filter.check_filter(&tx) {
                                    txes.push(tx);
                                    if txes.len() >= page.limit {
                                        done = true
                                    }
                                }
                            }
                        }
                    }
                    Err(_) => {}
                },
                None => done = true
            }
        }

        let result = PageResult {
            values: txes,
            cursor: None,
        };

        Ok(result)
    }

    fn get_tx(&self, blockchain: u32, txid: &str) -> Option<proto_Transaction> {
        let key = TransactionsAccess::get_key(blockchain, txid);
        self.get_tx_by_key(key)
    }

    fn get_tx_meta(&self, blockchain: u32, txid: &str) -> Result<Option<proto_TransactionMeta>, StateError> {
        let key = TransactionsAccess::get_key_meta(blockchain, txid);
        match self.db.get(key) {
            Ok(data) => {
                match data {
                    Some(b) => Ok(proto_TransactionMeta::parse_from_bytes(b.deref()).ok()),
                    None => Ok(None)
                }
            }
            Err(_) => Err(StateError::IOError)
        }
    }

    fn set_tx_meta(&self, value: proto_TransactionMeta) -> Result<proto_TransactionMeta, StateError> {
        let blockchain = value.blockchain.value() as u32;
        let tx_id = value.tx_id.clone();
        if tx_id.is_empty() {
            return Err(StateError::InvalidValue(InvalidValueError::Name("tx_id".to_string())))
        }
        let existing = self.get_tx_meta(blockchain, tx_id.as_str())?;
        if let Some(existing_value) = existing {
            if existing_value.timestamp >= value.timestamp {
                return Ok(existing_value)
            }
        }
        let key = TransactionsAccess::get_key_meta(blockchain, tx_id);
        let b = value.write_to_bytes()?;
        let mut batch = Batch::default();
        batch.insert(key.as_bytes(), b);
        self.db.apply_batch(batch)?;
        Ok(value)
    }

    fn submit(&self, transactions: Vec<proto_Transaction>) -> Result<(), StateError> {
        let mut batch = Batch::default();
        for tx in transactions {
            if let Ok(tx_bytes) = tx.write_to_bytes() {
                let tx_id = tx.tx_id.clone();
                let tx_key = TransactionsAccess::get_key(tx.blockchain.value() as u32, tx_id);
                let indexes: Vec<String> = tx.get_index_keys();
                Indexing::add_backrefs(&indexes, tx_key.clone(), &mut batch)?;
                for idx in indexes {
                    batch.insert(idx.as_bytes(), tx_key.as_bytes());
                }
                batch.insert(tx_key.as_bytes(), tx_bytes);
            }
        }
        self.db.apply_batch(batch)
            .map_err(|e| StateError::from(e))
    }

    fn forget(&self, blockchain: u32, tx_id: String) -> Result<(), StateError> {
        let mut batch = Batch::default();
        let tx_key = TransactionsAccess::get_key(blockchain, tx_id);
        batch.remove(tx_key.as_bytes());
        Indexing::remove_backref(tx_key, self.db.clone(), &mut batch)?;
        self.db.apply_batch(batch)
            .map_err(|e| StateError::from(e))
    }

    fn get_count(&self, filter: Filter) -> Result<usize, StateError> {
        let bounds = filter.get_index_bounds();
        let mut processed = HashSet::new();
        let mut iter = self.db.range(bounds);
        let mut count = 0;
        let mut done = false;
        while !done {
            match iter.next() {
                Some(x) => {
                    match x {
                        Ok(v) => {
                            let txkey = v.1.to_vec();
                            let txkey = String::from_utf8(txkey).unwrap();
                            let unprocessed = processed.insert(txkey.clone());
                            if unprocessed {
                                if let Some(tx) = self.get_tx_by_key(txkey) {
                                    if filter.check_filter(&tx) {
                                        count += 1;
                                    }
                                }
                            }
                        }
                        Err(_) => {}
                    }
                }
                None => done = true
            }
        }
        Ok(count)
    }

    fn get_cursor<S: AsRef<str>>(&self, address: S) -> Result<Option<RemoteCursor>, StateError> {
        let key = format!("{}:{}", PREFIX_CURSOR, address.as_ref());
        if let Some(value) = self.db.get(key)? {
            let cursor = proto_Cursor::parse_from_bytes(value.deref())?;
            if cursor.value.is_empty() {
                Ok(None)
            } else {
                Ok(Some(RemoteCursor {
                    value: cursor.value,
                    since: Utc.timestamp_millis(cursor.ts as i64)
                }))
            }
        } else {
            Ok(None)
        }
    }

    fn set_cursor<S: AsRef<str> + ToString>(&self, address: S, cursor: S) -> Result<(), StateError> {
        let key = format!("{}:{}", PREFIX_CURSOR, address.as_ref());
        let mut proto = proto_Cursor::new();
        proto.set_address(address.to_string());
        proto.set_ts(Utc::now().naive_utc().timestamp_millis() as u64);
        proto.set_value(cursor.to_string());
        let value = proto.write_to_bytes()?;
        let mut batch = Batch::default();
        batch.insert(key.as_bytes(), value.as_slice());
        self.db.apply_batch(batch)
            .map_err(|e| StateError::from(e))
    }
}

#[cfg(test)]
mod tests {
    use tempdir::TempDir;
    use std::str::FromStr;
    use chrono::Utc;
    use uuid::Uuid;
    use crate::access::transactions::{AddressRef, Filter, Transactions, WalletRef};
    use crate::access::pagination::PageQuery;
    use crate::storage::transaction_store::{IndexType, IndexedValue};
    use crate::proto::transactions::{BlockchainId, Transaction as proto_Transaction, Change as proto_Change, TransactionMeta as proto_TransactionMeta};
    use crate::storage::indexing::IndexEncoding;
    use crate::storage::sled_access::SledStorage;

    #[test]
    fn get_index_at_ts() {
        let idx = IndexType::Everything(1_647_313_850_992);
        assert_eq!("idx:tx:1/D8352686149007", idx.get_index_key());
    }

    #[test]
    fn get_index_at_wallet() {
        let idx = IndexType::ByWallet(Uuid::from_str("72279ede-44c4-4951-925b-f51a7b9e929a").unwrap(), 1_647_313_850_992);
        assert_eq!("idx:tx:2/72279ede-44c4-4951-925b-f51a7b9e929a/D8352686149007", idx.get_index_key());
    }

    #[test]
    fn build_indexes_basic() {
        let mut tx = proto_Transaction::new();
        tx.blockchain = BlockchainId::CHAIN_ETHEREUM;
        tx.since_timestamp = 1_647_313_850_992;
        let mut change1 = proto_Change::new();
        change1.wallet_id = "72279ede-44c4-4951-925b-f51a7b9e929a".to_string();
        change1.entry_id = 0;
        change1.address = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_string();
        tx.changes.push(change1);

        let indexes: Vec<String> = tx.get_index_keys();
        assert_eq!(indexes.len(), 2);
        assert_eq!("idx:tx:1/D8352686149007", indexes.get(0).unwrap());
        assert_eq!("idx:tx:2/72279ede-44c4-4951-925b-f51a7b9e929a/D8352686149007", indexes.get(1).unwrap());
    }

    #[test]
    fn create_and_find_tx() {
        let tmp_dir = TempDir::new("create_and_find_tx").unwrap();
        let access = SledStorage::open(tmp_dir.path().to_path_buf()).unwrap();
        let transactions = access.get_transactions();

        let mut tx = proto_Transaction::new();
        tx.blockchain = BlockchainId::CHAIN_ETHEREUM;
        tx.tx_id = "0x2f761cbf069962cf3a82ab0d9b11c453e5d0caf4fb6d192624360def7bd1e81b".to_string();
        tx.since_timestamp = 1_647_313_850_992;
        let mut change1 = proto_Change::new();
        change1.wallet_id = "72279ede-44c4-4951-925b-f51a7b9e929a".to_string();
        change1.entry_id = 0;
        change1.address = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_string();
        tx.changes.push(change1);

        transactions.submit(vec![tx.clone()]).expect("not saved");

        let results = transactions.query(Filter::default(), PageQuery::default()).expect("queried");
        assert_eq!(results.values.len(), 1);
        assert_eq!(results.values.get(0).unwrap().clone(), tx);
        assert!(results.cursor.is_none());
    }

    #[test]
    fn create_and_delete_tx() {
        let tmp_dir = TempDir::new("create_and_find_tx").unwrap();
        let access = SledStorage::open(tmp_dir.path().to_path_buf()).unwrap();
        let transactions = access.get_transactions();

        let mut tx = proto_Transaction::new();
        tx.blockchain = BlockchainId::CHAIN_ETHEREUM;
        tx.tx_id = "0x2f761cbf069962cf3a82ab0d9b11c453e5d0caf4fb6d192624360def7bd1e81b".to_string();
        tx.since_timestamp = 1_647_313_850_992;
        let mut change1 = proto_Change::new();
        change1.wallet_id = "72279ede-44c4-4951-925b-f51a7b9e929a".to_string();
        change1.entry_id = 0;
        change1.address = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_string();
        tx.changes.push(change1);

        transactions.submit(vec![tx.clone()]).expect("not saved");

        let results = transactions.query(Filter::default(), PageQuery::default()).expect("queried");
        assert_eq!(results.values.len(), 1);

        transactions.forget(100, "0x2f761cbf069962cf3a82ab0d9b11c453e5d0caf4fb6d192624360def7bd1e81b".to_string()).expect("not removed");
        let results = transactions.query(Filter::default(), PageQuery::default()).expect("queried");
        assert_eq!(results.values.len(), 0);

        let db_size = access.db.scan_prefix("").count();
        assert_eq!(db_size, 0);
    }

    #[test]
    fn loads_using_desc_order() {
        let tmp_dir = TempDir::new("create_and_find_tx").unwrap();
        let access = SledStorage::open(tmp_dir.path().to_path_buf()).unwrap();
        let transactions = access.get_transactions();

        let mut tx1 = proto_Transaction::new();
        tx1.blockchain = BlockchainId::CHAIN_ETHEREUM;
        tx1.tx_id = "0xd9b11cef7bd1e81b453e5d0caf4fb6d1922f761cbf069962cf3a82ab0624360d".to_string();
        tx1.since_timestamp = 1_647_313_000_000;
        let mut change1 = proto_Change::new();
        change1.wallet_id = "72279ede-44c4-4951-925b-f51a7b9e929a".to_string();
        change1.entry_id = 0;
        change1.address = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_string();
        tx1.changes.push(change1);

        let mut tx2 = proto_Transaction::new();
        tx2.blockchain = BlockchainId::CHAIN_ETHEREUM;
        tx2.tx_id = "0x2f761cbf069962cf3a82ab0d9b11c453e5d0caf4fb6d192624360def7bd1e81b".to_string();
        tx2.since_timestamp = 1_647_315_000_000;
        let mut change1 = proto_Change::new();
        change1.wallet_id = "72279ede-44c4-4951-925b-f51a7b9e929a".to_string();
        change1.entry_id = 0;
        change1.address = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_string();
        tx2.changes.push(change1);

        transactions.submit(vec![tx1.clone(), tx2.clone()]).expect("not saved");

        let results = transactions.query(Filter::default(), PageQuery::default()).expect("query data");
        assert_eq!(results.values.len(), 2);
        assert_eq!(results.values.get(0).unwrap().clone(), tx2);
        assert_eq!(results.values.get(1).unwrap().clone(), tx1);
        assert!(results.cursor.is_none());
    }

    #[test]
    fn count_items() {
        let tmp_dir = TempDir::new("create_and_find_tx").unwrap();
        let access = SledStorage::open(tmp_dir.path().to_path_buf()).unwrap();
        let transactions = access.get_transactions();

        let mut tx1 = proto_Transaction::new();
        tx1.blockchain = BlockchainId::CHAIN_ETHEREUM;
        tx1.tx_id = "0xd9b11cef7bd1e81b453e5d0caf4fb6d1922f761cbf069962cf3a82ab0624360d".to_string();
        tx1.since_timestamp = 1_647_313_000_000;
        let mut change1 = proto_Change::new();
        change1.wallet_id = "72279ede-44c4-4951-925b-f51a7b9e929a".to_string();
        change1.entry_id = 0;
        change1.address = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_string();
        tx1.changes.push(change1);

        let mut tx2 = proto_Transaction::new();
        tx2.blockchain = BlockchainId::CHAIN_ETHEREUM;
        tx2.tx_id = "0x2f761cbf069962cf3a82ab0d9b11c453e5d0caf4fb6d192624360def7bd1e81b".to_string();
        tx2.since_timestamp = 1_647_315_000_000;
        let mut change1 = proto_Change::new();
        change1.wallet_id = "72279ede-44c4-4951-925b-f51a7b9e929a".to_string();
        change1.entry_id = 0;
        change1.address = "0x6218b36c1d19d4a2e9eb0ce3606eb48a0b86991c".to_string();
        tx2.changes.push(change1);

        transactions.submit(vec![tx1.clone(), tx2.clone()]).expect("not saved");

        let count = transactions.get_count(Filter::default()).expect("query count");
        assert_eq!(count, 2);

        let count = transactions.get_count(Filter {
            addresses: Some(vec![AddressRef::SingleAddress("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_string())]),
            ..Filter::default()
        }).expect("query count");
        assert_eq!(count, 1);

        let count = transactions.get_count(Filter {
            addresses: Some(vec![AddressRef::SingleAddress("0x6218b36c1d19d4a2e9eb0ce3606eb48a0b86991c".to_string())]),
            ..Filter::default()
        }).expect("query count");
        assert_eq!(count, 1);

        let count = transactions.get_count(Filter {
            wallet: Some(WalletRef::WholeWallet(Uuid::from_str("72279ede-44c4-4951-925b-f51a7b9e929a").unwrap())),
            ..Filter::default()
        }).expect("query count");
        assert_eq!(count, 2);
    }

    #[test]
    fn no_cursor_by_default() {
        let tmp_dir = TempDir::new("create_and_find_tx").unwrap();
        let access = SledStorage::open(tmp_dir.path().to_path_buf()).unwrap();
        let transactions = access.get_transactions();

        let act = transactions.get_cursor("0x6218b36c1d19d4a2e9eb0ce3606eb48a0b86991c");
        assert!(act.is_ok());
        assert!(act.unwrap().is_none());
    }

    #[test]
    fn save_and_provide_cursor() {
        let tmp_dir = TempDir::new("create_and_find_tx").unwrap();
        let access = SledStorage::open(tmp_dir.path().to_path_buf()).unwrap();
        let transactions = access.get_transactions();

        let saved = transactions.set_cursor("0x6218b36c1d19d4a2e9eb0ce3606eb48a0b86991c", "MTA5MjQ5MS81ODE=");
        assert!(saved.is_ok());

        let act = transactions.get_cursor("0x6218b36c1d19d4a2e9eb0ce3606eb48a0b86991c");
        assert!(act.is_ok());
        let act = act.unwrap();
        assert!(act.is_some());
        assert_eq!(act.unwrap().value, "MTA5MjQ5MS81ODE=".to_string());
    }

    #[test]
    fn no_tx_meta_by_default() {
        let tmp_dir = TempDir::new("tx").unwrap();
        let access = SledStorage::open(tmp_dir.path().to_path_buf()).unwrap();
        let transactions = access.get_transactions();

        let act = transactions.get_tx_meta(100, "0x2f761cbf069962cf3a82ab0d9b11c453e5d0caf4fb6d192624360def7bd1e81b");

        assert!(act.is_ok());
        let act = act.unwrap();
        assert!(act.is_none());
    }

    #[test]
    fn set_and_get_tx_meta() {
        let tmp_dir = TempDir::new("tx").unwrap();
        let access = SledStorage::open(tmp_dir.path().to_path_buf()).unwrap();
        let transactions = access.get_transactions();

        let mut meta = proto_TransactionMeta::new();
        meta.blockchain = BlockchainId::CHAIN_ETHEREUM;
        meta.tx_id = "0x2f761cbf069962cf3a82ab0d9b11c453e5d0caf4fb6d192624360def7bd1e81b".to_string();
        meta.timestamp = 1_647_313_850_992;
        meta.label = "test".to_string();
        transactions.set_tx_meta(meta.clone()).unwrap();

        let act = transactions.get_tx_meta(100, "0x2f761cbf069962cf3a82ab0d9b11c453e5d0caf4fb6d192624360def7bd1e81b");

        assert!(act.is_ok());
        let act = act.unwrap();
        assert!(act.is_some());
        let act = act.unwrap();
        assert_eq!(act, meta);
    }

    #[test]
    fn update_tx_meta_to_latest() {
        let tmp_dir = TempDir::new("tx").unwrap();
        let access = SledStorage::open(tmp_dir.path().to_path_buf()).unwrap();
        let transactions = access.get_transactions();

        let mut meta = proto_TransactionMeta::new();
        meta.blockchain = BlockchainId::CHAIN_ETHEREUM;
        meta.tx_id = "0x2f761cbf069962cf3a82ab0d9b11c453e5d0caf4fb6d192624360def7bd1e81b".to_string();
        meta.timestamp = 1_647_313_000_000;
        meta.label = "test".to_string();
        transactions.set_tx_meta(meta.clone()).unwrap();

        meta.timestamp = 1_647_313_100_000;
        meta.label = "test 2".to_string();
        transactions.set_tx_meta(meta.clone()).unwrap();

        let act = transactions
            .get_tx_meta(100, "0x2f761cbf069962cf3a82ab0d9b11c453e5d0caf4fb6d192624360def7bd1e81b")
            .unwrap().unwrap();

        assert_eq!(act.label, "test 2");
    }

    #[test]
    fn no_update_tx_meta_to_old() {
        let tmp_dir = TempDir::new("tx").unwrap();
        let access = SledStorage::open(tmp_dir.path().to_path_buf()).unwrap();
        let transactions = access.get_transactions();

        let mut meta = proto_TransactionMeta::new();
        meta.blockchain = BlockchainId::CHAIN_ETHEREUM;
        meta.tx_id = "0x2f761cbf069962cf3a82ab0d9b11c453e5d0caf4fb6d192624360def7bd1e81b".to_string();
        meta.timestamp = 1_647_313_100_000;
        meta.label = "test 1".to_string();
        transactions.set_tx_meta(meta.clone()).unwrap();

        meta.timestamp = 1_647_313_000_000;
        meta.label = "test 2".to_string();
        transactions.set_tx_meta(meta.clone()).unwrap();

        let act = transactions
            .get_tx_meta(100, "0x2f761cbf069962cf3a82ab0d9b11c453e5d0caf4fb6d192624360def7bd1e81b")
            .unwrap().unwrap();

        assert_eq!(act.label, "test 1");
    }
}