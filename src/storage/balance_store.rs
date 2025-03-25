use std::sync::Arc;
use protobuf::Message;
use sled::{Db, IVec};
use crate::access::balance::{Balance, Balances, concat};
use crate::errors::{StateError};
use crate::proto::balance::{BalanceBundle as proto_BalanceBundle};
use crate::{validate};
use crate::storage::version::Migration;

const PREFIX_KEY: &'static str = "balance:";

pub struct BalanceAccess {
    pub(crate) db: Arc<Db>,
}

impl BalanceAccess {
    fn get_key(addr: &String) -> String {
        format!("{}{}", PREFIX_KEY, addr.to_string())
    }

    fn convert_stored(base: IVec) -> Vec<Balance> {
        match proto_BalanceBundle::parse_from_bytes(base.as_ref()) {
            Ok(parsed) => parsed.into(),
            Err(_) => vec![]
        }
    }
}

impl Migration for BalanceAccess {
    fn migrate(&self, version: usize) -> Result<(), StateError> {
        if version == 1 {
            // before version 1 we may stored some balances without a token and the wallet may show some outdated information, or
            // information that doesn't exist and therefore cannot be updated by wallet.
            // Here we just remove all balances, because wallet will reload all actual balances anyway.
            self.db.scan_prefix(PREFIX_KEY.as_bytes()).keys().for_each(|k| {
                if let Ok(key) = k {
                    let _ = self.db.remove(key);
                }
            });
        }
        Ok(())
    }
}

impl Balances for BalanceAccess {

    fn set(&self, value: Balance) -> Result<(), StateError> {
        validate::check_address(&value.address)?;

        let key = BalanceAccess::get_key(&value.address);
        let value = if let Some(base) = self.db.get(&key)? {
            let base: Vec<Balance> = BalanceAccess::convert_stored(base);
            concat(base, value)
        } else {
            vec![value]
        };
        let value: proto_BalanceBundle = value.into();
        let bytes = value.write_to_bytes()?;
        self.db.insert(key.as_bytes(), bytes)?;

        Ok(())
    }

    fn list(&self, address: String) -> Result<Vec<Balance>, StateError> {
        validate::check_address(&address)?;

        let key = BalanceAccess::get_key(&address);
        let value = if let Some(base) = self.db.get(&key)? {
            BalanceAccess::convert_stored(base)
        } else {
            vec![]
        };
        Ok(value)
    }

    fn clear(&self, address: String) -> Result<(), StateError> {
        validate::check_address(&address)?;

        let key = BalanceAccess::get_key(&address);
        self.db.remove(key.as_bytes())?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use chrono::{TimeZone, Utc};
    use num_bigint::BigUint;
    use tempdir::TempDir;
    use crate::access::balance::{Balance, Balances, Utxo};
    use crate::storage::sled_access::SledStorage;

    #[test]
    fn list_nothing_for_new() {
        let tmp_dir = TempDir::new("balance").unwrap();
        let access = SledStorage::open(tmp_dir.path().to_path_buf()).unwrap();
        let balances = access.get_balance();

        let act = balances.list("0xCD2a3d9F938E13CD947Ec05AbC7FE734Df8DD826".to_string());

        assert!(act.is_ok());
        let act = act.unwrap();
        assert!(act.is_empty());
    }

    #[test]
    fn list_just_added() {
        let tmp_dir = TempDir::new("balance").unwrap();
        let access = SledStorage::open(tmp_dir.path().to_path_buf()).unwrap();
        let balances = access.get_balance();

        let balance0 = Balance {
            address: "0xCD2a3d9F938E13CD947Ec05AbC7FE734Df8DD826".to_string(),
            blockchain: 100,
            asset: "ETHER".to_string(),
            amount: BigUint::from(100u32),
            ts: Utc.timestamp_millis_opt(1675123456789).unwrap(),
            ..Balance::default()
        };

        let added = balances.set(balance0.clone());
        assert!(added.is_ok());

        let act = balances.list("0xCD2a3d9F938E13CD947Ec05AbC7FE734Df8DD826".to_string());

        assert!(act.is_ok());
        let act = act.unwrap();
        assert_eq!(act.len(), 1);
        assert_eq!(act[0], balance0);
    }

    #[test]
    fn keeps_multiple_assets() {
        let tmp_dir = TempDir::new("balance").unwrap();
        let access = SledStorage::open(tmp_dir.path().to_path_buf()).unwrap();
        let balances = access.get_balance();

        let balance0 = Balance {
            address: "0xCD2a3d9F938E13CD947Ec05AbC7FE734Df8DD826".to_string(),
            blockchain: 100,
            asset: "ETHER".to_string(),
            amount: BigUint::from(100u32),
            ts: Utc.timestamp_millis_opt(1675123456789).unwrap(),
            ..Balance::default()
        };

        let balance1 = Balance {
            address: "0xCD2a3d9F938E13CD947Ec05AbC7FE734Df8DD826".to_string(),
            blockchain: 100,
            asset: "ERC20:0xdAC17F958D2ee523a2206206994597C13D831ec7".to_string(),
            amount: BigUint::from(200u32),
            ts: Utc.timestamp_millis_opt(1675123456789).unwrap(),
            ..Balance::default()
        };

        let added = balances.set(balance0.clone());
        assert!(added.is_ok());
        let added = balances.set(balance1.clone());
        assert!(added.is_ok());

        let act = balances.list("0xCD2a3d9F938E13CD947Ec05AbC7FE734Df8DD826".to_string());

        assert!(act.is_ok());
        let act = act.unwrap();
        assert_eq!(act.len(), 2);
        assert_eq!(act[0], balance0);
        assert_eq!(act[1], balance1);
    }

    #[test]
    fn replace_with_new() {
        let tmp_dir = TempDir::new("balance").unwrap();
        let access = SledStorage::open(tmp_dir.path().to_path_buf()).unwrap();
        let balances = access.get_balance();

        let balance0 = Balance {
            address: "12cbQLTFMXRnSzktFkuoG3eHoMeFtpTu3S".to_string(),
            blockchain: 1,
            asset: "BTC".to_string(),
            amount: BigUint::from(1000u32),
            ts: Utc.timestamp_millis_opt(1675123456789).unwrap(),
            ..Balance::default()
        };

        let balance1 = Balance {
            address: "12cbQLTFMXRnSzktFkuoG3eHoMeFtpTu3S".to_string(),
            blockchain: 1,
            asset: "BTC".to_string(),
            amount: BigUint::from(2000u32),
            ts: Utc.timestamp_millis_opt(1675123456789).unwrap(),
            ..Balance::default()
        };

        let added = balances.set(balance0.clone());
        assert!(added.is_ok());
        let added = balances.set(balance1.clone());
        assert!(added.is_ok());

        let act = balances.list("12cbQLTFMXRnSzktFkuoG3eHoMeFtpTu3S".to_string());

        assert!(act.is_ok());
        let act = act.unwrap();
        assert_eq!(act.len(), 1);
        assert_eq!(act[0], balance1);
    }

    #[test]
    fn remove_added() {
        let tmp_dir = TempDir::new("balance").unwrap();
        let access = SledStorage::open(tmp_dir.path().to_path_buf()).unwrap();
        let balances = access.get_balance();

        let balance0 = Balance {
            address: "0xCD2a3d9F938E13CD947Ec05AbC7FE734Df8DD826".to_string(),
            blockchain: 100,
            asset: "ETHER".to_string(),
            amount: BigUint::from(100u32),
            ts: Utc.timestamp_millis_opt(1675123456789).unwrap(),
            ..Balance::default()
        };

        balances.set(balance0.clone()).unwrap();

        let added = balances.list("0xCD2a3d9F938E13CD947Ec05AbC7FE734Df8DD826".to_string()).unwrap();
        assert_eq!(added.len(), 1);

        let removed = balances.clear("0xCD2a3d9F938E13CD947Ec05AbC7FE734Df8DD826".to_string());
        assert!(removed.is_ok());

        let act = balances.list("0xCD2a3d9F938E13CD947Ec05AbC7FE734Df8DD826".to_string()).unwrap();
        assert_eq!(act.len(), 0);
    }

    #[test]
    fn store_utxo() {
        let tmp_dir = TempDir::new("balance").unwrap();
        let access = SledStorage::open(tmp_dir.path().to_path_buf()).unwrap();
        let balances = access.get_balance();

        let balance0 = Balance {
            address: "bc1qywz558j2ja7fwmg32jupn02qvla5zm3dvggpqv".to_string(),
            blockchain: 1,
            asset: "BTC".to_string(),
            amount: BigUint::from(23045u64),
            ts: Utc.timestamp_millis_opt(1675123456789).unwrap(),
            utxo: vec![
                Utxo {
                    txid: "01ff3e2b6d2f1e52aa548e79b8f43d0091e9541bc4f70cda4e6549aaf836268b".to_string(),
                    vout: 1,
                    amount: 23045
                }
            ],
            ..Balance::default()
        };

        let added = balances.set(balance0.clone());
        assert!(added.is_ok());

        let act = balances.list("bc1qywz558j2ja7fwmg32jupn02qvla5zm3dvggpqv".to_string());

        assert!(act.is_ok());
        let act = act.unwrap();
        assert_eq!(act.len(), 1);
        assert_eq!(act[0].utxo.len(), 1);
        assert_eq!(act[0].utxo[0], Utxo {
            txid: "01ff3e2b6d2f1e52aa548e79b8f43d0091e9541bc4f70cda4e6549aaf836268b".to_string(),
            vout: 1,
            amount: 23045
        });
    }

    #[test]
    fn ignore_invalid_utxo() {
        let tmp_dir = TempDir::new("balance").unwrap();
        let access = SledStorage::open(tmp_dir.path().to_path_buf()).unwrap();
        let balances = access.get_balance();

        let balance0 = Balance {
            address: "bc1qywz558j2ja7fwmg32jupn02qvla5zm3dvggpqv".to_string(),
            blockchain: 1,
            asset: "BTC".to_string(),
            amount: BigUint::from(23045u64),
            ts: Utc.timestamp_millis_opt(1675123456789).unwrap(),
            utxo: vec![
                Utxo {
                    txid: "01ff3e2b6d2f1e52aa548e79b8f43d0091e9541bc4f70cda4e6549aaf836268b".to_string(),
                    vout: 1,
                    amount: 12345
                }
            ],
            ..Balance::default()
        };

        let added = balances.set(balance0.clone());
        assert!(added.is_ok());

        let act = balances.list("bc1qywz558j2ja7fwmg32jupn02qvla5zm3dvggpqv".to_string());

        assert!(act.is_ok());
        let act = act.unwrap();
        assert_eq!(act.len(), 1);
        assert_eq!(act[0].utxo.len(), 0);
    }
}