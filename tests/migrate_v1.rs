use std::fs;
use std::io::Cursor;
use std::path::PathBuf;
use protobuf::ProtobufEnum;
use tempdir::TempDir;
use emerald_wallet_state::access::balance::Balances;
use emerald_wallet_state::access::cache::Cache;
use emerald_wallet_state::access::transactions::Transactions;
use emerald_wallet_state::proto::transactions::BlockchainId;
use emerald_wallet_state::storage::sled_access::SledStorage;

#[test]
fn migrate_from_v0() {
    let tmp_dir = TempDir::new("migrate_from_v0").unwrap();
    let archive = PathBuf::from("testdata/basic_v0.zip");
    let archive = fs::read(archive).unwrap();
    zip_extract::extract(Cursor::new(archive), &tmp_dir.path().to_path_buf(), false).unwrap();

    let store = SledStorage::open(tmp_dir.path().to_path_buf()).unwrap();

    let cache = store.get_cache();
    let value = cache.get("test".to_string());
    assert!(value.is_ok());
    let value = value.unwrap();
    assert_eq!(value, Some("Test".to_string()));

    let transactions = store.get_transactions();
    let value = transactions.get_tx(
        BlockchainId::CHAIN_ETHEREUM.value() as u32,
        "0x2f761cbf069962cf3a82ab0d9b11c453e5d0caf4fb6d192624360def7bd1e81b"
    );
    assert!(value.is_some());

    let balances = store.get_balance();
    let values = balances.list("bc1qywz558j2ja7fwmg32jupn02qvla5zm3dvggpqv".to_string()).unwrap();
    assert_eq!(values.len(), 0);
}