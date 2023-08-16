use std::sync::Arc;
use sled::Db;
use crate::errors::StateError;
use crate::storage::balance_store::BalanceAccess;
use crate::storage::transaction_store::TransactionsAccess;

const KEY: &'static str = "version";
const CURRENT_VERSION: usize = 1usize;

pub struct Version {
    db: Arc<Db>,
}

pub(crate) trait Migration {
    fn migrate(&self, version: usize) -> Result<(), StateError>;
}

///
/// Manage DB version
///
impl Version {
    pub(crate) fn new(db: Arc<Db>) -> Self {
        Version { db }
    }

    ///
    /// Get current DB version. If version is not set, returns None
    ///
    pub fn get_version(&self) -> Result<Option<usize>, StateError> {
        let version = self.db.get(KEY)?;
        match version {
            Some(v) => if let Ok(version) = String::from_utf8(v.to_vec()) {
                if let Ok(version) = version.parse::<usize>() {
                    return Ok(Some(version));
                } else {
                    Ok(None)
                }
            } else {
                Ok(None)
            },
            None => Ok(None),
        }
    }

    fn set_version(&self, version: usize) -> Result<(), StateError> {
        self.db.insert(KEY, format!("{}", version).as_bytes())?;
        Ok(())
    }

    ///
    /// Migrate DB to the latest version. May include a deletion of some data.
    ///
    pub fn migrate(&self) -> Result<(), StateError> {
        let act = self.get_version()?;
        if act.is_none() || act.unwrap() < CURRENT_VERSION {
            let balances = BalanceAccess { db: self.db.clone() };
            balances.migrate(CURRENT_VERSION)?;

            let transactions = TransactionsAccess { db: self.db.clone() };
            transactions.migrate(CURRENT_VERSION)?;

            self.set_version(CURRENT_VERSION)?;
        }
        Ok(())
    }
}
