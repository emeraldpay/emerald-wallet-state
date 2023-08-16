use chrono::{Utc, TimeZone};
use num_bigint::BigUint;
use tempdir::TempDir;
use emerald_wallet_state::access::balance::{Balance, Balances, Utxo};
use emerald_wallet_state::access::cache::Cache;
use emerald_wallet_state::access::transactions::Transactions;
use emerald_wallet_state::storage::sled_access::SledStorage;
use emerald_wallet_state::proto::transactions::{
    BlockchainId,
    Transaction as proto_Transaction,
    Change as proto_Change,
    Direction,
    Change_ChangeType,
};

#[test]
fn save_values() {
    let tmp_dir = TempDir::new("save_values").unwrap();
    {
        let store = SledStorage::open(tmp_dir.path().to_path_buf()).unwrap();
        let mut cache = store.get_cache();
        let _ = cache.put("test".to_string(), "Test".to_string(), None).unwrap();
    }

    let store = SledStorage::open(tmp_dir.path().to_path_buf()).unwrap();
    let cache = store.get_cache();
    let value = cache.get("test".to_string());
    assert!(value.is_ok());
    let value = value.unwrap();
    assert_eq!(value, Some("Test".to_string()));
}

#[test]
fn write_multiple() {
    let tmp_dir = TempDir::new("write_multiple").unwrap();
    let store = SledStorage::open(tmp_dir.path().to_path_buf()).unwrap();

    let mut cache = store.get_cache();
    let saved = cache.put("test".to_string(), "Test".to_string(), None);
    assert!(saved.is_ok());

    let transactions = store.get_transactions();
    let mut tx = proto_Transaction::new();
    tx.blockchain = BlockchainId::CHAIN_ETHEREUM;
    tx.tx_id = "0x2f761cbf069962cf3a82ab0d9b11c453e5d0caf4fb6d192624360def7bd1e81b".to_string();
    tx.since_timestamp = 1_647_313_850_992;
    let mut change1 = proto_Change::new();
    change1.wallet_id = "72279ede-44c4-4951-925b-f51a7b9e929a".to_string();
    change1.entry_id = 0;
    change1.address = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_string();
    change1.amount = "100".to_string();
    change1.direction = Direction::SEND;
    change1.change_type = Change_ChangeType::TRANSFER;
    tx.changes.push(change1.clone());
    let saved = transactions.submit(vec![tx]);
    assert!(saved.is_ok());

    let balances = store.get_balance();
    let balance = Balance {
        address: "bc1qywz558j2ja7fwmg32jupn02qvla5zm3dvggpqv".to_string(),
        blockchain: 1,
        asset: "BTC".to_string(),
        amount: BigUint::from(23045u64),
        ts: Utc.timestamp_millis(1675123456789),
        utxo: vec![
            Utxo {
                txid: "01ff3e2b6d2f1e52aa548e79b8f43d0091e9541bc4f70cda4e6549aaf836268b".to_string(),
                vout: 1,
                amount: 23045
            }
        ],
        ..Balance::default()
    };
    let saved = balances.set(balance);
    assert!(saved.is_ok());
}