use std::sync::Arc;
use protobuf::Message;
use sled::{Db, IVec};
use crate::access::balance::{Balance, Balances, concat};
use crate::errors::{InvalidValueError, StateError};
use crate::proto::balance::{BalanceBundle as proto_BalanceBundle};

const PREFIX_KEY: &'static str = "balance:";

pub struct BalanceAccess {
    pub(crate) db: Arc<Db>,
}

impl BalanceAccess {
    fn get_key(addr: &String) -> String {
        format!("{}{}", PREFIX_KEY, addr.to_string())
    }

    fn validate_address(address: &String) -> Result<(), StateError> {
        if !address.is_ascii() {
            return Err(StateError::InvalidValue(
                InvalidValueError::NameMessage("address".to_string(), "non-ascii".to_string())))
        }
        Ok(())
    }

    fn convert_stored(base: IVec) -> Vec<Balance> {
        match proto_BalanceBundle::parse_from_bytes(base.as_ref()) {
            Ok(parsed) => parsed.into(),
            Err(_) => vec![]
        }
    }
}



impl Balances for BalanceAccess {

    fn set(&self, value: Balance) -> Result<(), StateError> {
        BalanceAccess::validate_address(&value.address)?;

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
        BalanceAccess::validate_address(&address)?;

        let key = BalanceAccess::get_key(&address);
        let value = if let Some(base) = self.db.get(&key)? {
            BalanceAccess::convert_stored(base)
        } else {
            vec![]
        };
        Ok(value)
    }

    fn clear(&self, address: String) -> Result<(), StateError> {
        BalanceAccess::validate_address(&address)?;

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
    use crate::access::balance::{Balance, Balances};
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
            ts: Utc.timestamp_millis(1675123456789)
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
            ts: Utc.timestamp_millis(1675123456789)
        };

        let balance1 = Balance {
            address: "0xCD2a3d9F938E13CD947Ec05AbC7FE734Df8DD826".to_string(),
            blockchain: 100,
            asset: "ERC20:0xdAC17F958D2ee523a2206206994597C13D831ec7".to_string(),
            amount: BigUint::from(200u32),
            ts: Utc.timestamp_millis(1675123456789)
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
            ts: Utc.timestamp_millis(1675123456789)
        };

        let balance1 = Balance {
            address: "12cbQLTFMXRnSzktFkuoG3eHoMeFtpTu3S".to_string(),
            blockchain: 1,
            asset: "BTC".to_string(),
            amount: BigUint::from(2000u32),
            ts: Utc.timestamp_millis(1675123456789)
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
            ts: Utc.timestamp_millis(1675123456789)
        };

        balances.set(balance0.clone()).unwrap();

        let added = balances.list("0xCD2a3d9F938E13CD947Ec05AbC7FE734Df8DD826".to_string()).unwrap();
        assert_eq!(added.len(), 1);

        let removed = balances.clear("0xCD2a3d9F938E13CD947Ec05AbC7FE734Df8DD826".to_string());
        assert!(removed.is_ok());

        let act = balances.list("0xCD2a3d9F938E13CD947Ec05AbC7FE734Df8DD826".to_string()).unwrap();
        assert_eq!(act.len(), 0);
    }

}