use std::path::PathBuf;
use std::sync::Arc;
use sled::{Db};
use crate::errors::StateError;
use crate::storage::adressbook_store::AddressBookAccess;
use crate::storage::allowance_store::AllowanceAccess;
use crate::storage::balance_store::BalanceAccess;
use crate::storage::cache_store::CacheAccess;
use crate::storage::default_path;
use crate::storage::transaction_store::{TransactionsAccess};
use crate::storage::version::Version;
use crate::storage::xpubpos_store::XPubPositionAccess;

pub struct SledStorage {
    pub(crate) db: Arc<Db>,
}

/// Sled backed storage
impl SledStorage {

    /// Open DB at the default path
    pub fn open_default() -> Result<SledStorage, StateError> {
        SledStorage::open(default_path())
    }

    /// Open DB at the specified path
    pub fn open(path: PathBuf) -> Result<SledStorage, StateError> {
        let db = Arc::new(sled::open(path)?);
        let version = Version::new(db.clone());
        if let Err(e) = version.migrate() {
            println!("Failed to migrate DB: {:?}", e);
        }
        Ok(SledStorage {
            db,
        })
    }

    ///
    /// Open API to access DB version
    pub fn version(&self) -> Version {
        Version::new(self.db.clone())
    }

    /// Open API to access transactions store
    pub fn get_transactions(&self) -> TransactionsAccess {
        return TransactionsAccess { db: self.db.clone() };
    }

    pub fn get_addressbook(&self) -> AddressBookAccess {
        return AddressBookAccess { db: self.db.clone(), xpub: Arc::new(self.get_xpub_pos()) }
    }

    pub fn get_xpub_pos(&self) -> XPubPositionAccess {
        return XPubPositionAccess { db: self.db.clone() }
    }

    ///
    /// Cache for address balances
    pub fn get_balance(&self) -> BalanceAccess {
        return BalanceAccess { db: self.db.clone() }
    }

    ///
    /// Generic persistent cache
    pub fn get_cache(&self) -> CacheAccess {
        return CacheAccess { db: self.db.clone() }
    }

    ///
    /// ERC20 Allowance Cache
    pub fn get_allowance(&self) -> AllowanceAccess {
        return AllowanceAccess { db: self.db.clone() }
    }
}