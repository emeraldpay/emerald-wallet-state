use uuid::Uuid;
use crate::access::pagination::{PageQuery, PageResult};
use crate::errors::StateError;
use crate::proto::addressbook::BookItem;

pub struct Filter {
    pub blockchain: Option<u32>,
}

pub trait AddressBook {

    ///
    /// Add a new record to the Address Book.
    /// If the record doesn't have an ID it threts it as a new Records, which means generating a new random ID for it.
    /// If the ID is set, then an existing record with that ID gets updated.
    /// Returns list of IDs of created/updated records.
    fn add(&self, items: Vec<BookItem>) -> Result<Vec<Uuid>, StateError>;

    ///
    /// Remove a record with the specified id, if it does exit. Otherwise does nothing, returns ok in both cases.
    fn remove(&self, id: Uuid) -> Result<(), StateError>;

    ///
    /// Query for records in storage using specified filter and page
    fn query(&self, filter: Filter, page: PageQuery) -> Result<PageResult<BookItem>, StateError>;

}

impl Filter {
    pub fn check_filter(&self, t: &BookItem) -> bool {
        if let Some(b) = self.blockchain {
            t.blockchain == b
        } else {
            true
        }
    }
}

impl Default for Filter {
    fn default() -> Self {
        Filter {
            blockchain: None
        }
    }
}