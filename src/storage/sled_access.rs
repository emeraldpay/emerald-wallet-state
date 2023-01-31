use std::path::PathBuf;
use std::sync::Arc;
use sled::{Db};
use crate::errors::StateError;
use crate::storage::adressbook_store::AddressBookAccess;
use crate::storage::balance_store::BalanceAccess;
use crate::storage::default_path;
use crate::storage::transaction_store::{TransactionsAccess};
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
        let db = sled::open(path)?;
        Ok(SledStorage {
            db: Arc::new(db),
        })
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

    pub fn get_balance(&self) -> BalanceAccess {
        return BalanceAccess { db: self.db.clone() }
    }
}