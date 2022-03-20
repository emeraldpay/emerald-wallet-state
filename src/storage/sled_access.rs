use std::path::PathBuf;
use std::sync::Arc;
use sled::{Db};
use crate::errors::StateError;
use crate::storage::transaction_store::{TransactionsAccess};

pub struct SledStorage {
    pub(crate) db: Arc<Db>,
}

/// Sled backed storage
impl SledStorage {
    /// Open Sled DB at the specified path
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
}