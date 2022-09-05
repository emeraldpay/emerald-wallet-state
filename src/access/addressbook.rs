use uuid::Uuid;
use crate::access::pagination::{PageQuery, PageResult};
use crate::errors::StateError;
use crate::proto::addressbook::BookItem;

pub struct Filter {
    /// Filter by blockchain id
    pub blockchain: Option<u32>,
    /// Filter by text containing in the label, decription or address itself
    pub text: Option<String>,
}

pub trait AddressBook {

    ///
    /// Add a new record to the Address Book.
    /// If the record doesn't have an ID it threats it as a new Records, which means generating a new random ID for it.
    /// If the ID is set, then an existing record with that ID gets updated.
    /// Returns list of IDs of created/updated records.
    fn add(&self, items: Vec<BookItem>) -> Result<Vec<Uuid>, StateError>;

    ///
    /// Get an item if it exists.
    /// Returns `Ok(Some)` when it exists, or `Ok(None)` if not. Or `Err(StateError)` if cannot read
    fn get(&self, id: Uuid) -> Result<Option<BookItem>, StateError>;

    ///
    /// Remove a record with the specified id, if it does exit. Otherwise does nothing, returns ok in both cases.
    fn remove(&self, id: Uuid) -> Result<(), StateError>;

    ///
    /// Query for records in storage using specified filter and page
    fn query(&self, filter: Filter, page: PageQuery) -> Result<PageResult<BookItem>, StateError>;

    ///
    /// Update the store Address Book item with new values
    fn update(&self, id: Uuid, update: BookItem) -> Result<(), StateError>;
}

impl BookItem {
    fn address_contains(&self, q: String) -> bool {
        if !self.has_address() {
            return false
        }
        self.get_address()
            .address.to_lowercase().contains(&q)
    }
}

impl Filter {
    pub fn check_filter(&self, t: &BookItem) -> bool {
        let by_blockchain = if let Some(b) = &self.blockchain {
            t.blockchain == *b
        } else {
            true
        };

        let by_text = if let Some(q) = &self.text {
            let q = q.to_lowercase().trim().to_string();
            t.label.to_lowercase().contains(&q) || t.address_contains(q)
        } else {
            true
        };

        by_blockchain && by_text
    }
}

impl Default for Filter {
    fn default() -> Self {
        Filter {
            blockchain: None,
            text: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Filter};
    use crate::proto::addressbook::{BookItem as proto_BookItem, Address as proto_Address};

    #[test]
    fn default_filter_accept_any() {
        let filter = Filter::default();

        let mut item = proto_BookItem::new();
        item.id = "989d7648-13e3-4cb9-acfb-85464f063b34".to_string();
        item.create_timestamp = 1_647_313_850_992;
        item.blockchain = 101;
        let mut address = proto_Address::new();
        address.address = "0xEdD91797204D3537fBaBDe0E0E42AaE99975f2Bb".to_string();
        item.set_address(address);

        assert!(filter.check_filter(&item));
    }

    #[test]
    fn filter_by_blockchain() {
        let filter = Filter {
            blockchain: Some(101),
            ..Filter::default()
        };

        let mut item = proto_BookItem::new();
        item.id = "989d7648-13e3-4cb9-acfb-85464f063b34".to_string();
        item.create_timestamp = 1_647_313_850_992;
        item.blockchain = 101;
        let mut address = proto_Address::new();
        address.address = "0xEdD91797204D3537fBaBDe0E0E42AaE99975f2Bb".to_string();
        item.set_address(address);

        assert!(filter.check_filter(&item));

        item.blockchain = 1;
        assert!(!filter.check_filter(&item));
    }

    #[test]
    fn filter_by_label() {
        let filter = Filter {
            text: Some("World".to_string()),
            ..Filter::default()
        };

        let mut item = proto_BookItem::new();
        item.id = "989d7648-13e3-4cb9-acfb-85464f063b34".to_string();
        item.create_timestamp = 1_647_313_850_992;
        item.blockchain = 101;
        item.label = "Hello World!".to_string();
        let mut address = proto_Address::new();
        address.address = "0xEdD91797204D3537fBaBDe0E0E42AaE99975f2Bb".to_string();
        item.set_address(address);

        assert!(filter.check_filter(&item));

        item.label = "".to_string();
        assert!(!filter.check_filter(&item));
    }

    #[test]
    fn filter_by_address() {
        let filter = Filter {
            text: Some("edd9".to_string()),
            ..Filter::default()
        };

        let mut item = proto_BookItem::new();
        item.id = "989d7648-13e3-4cb9-acfb-85464f063b34".to_string();
        item.create_timestamp = 1_647_313_850_992;
        item.blockchain = 101;
        let mut address = proto_Address::new();
        address.address = "0xEdD91797204D3537fBaBDe0E0E42AaE99975f2Bb".to_string();
        item.set_address(address.clone());

        assert!(filter.check_filter(&item));

        address.address = "0x6e4a1797204D3537fBaBDe0E0E42AaE99975f2Bb".to_string();
        item.set_address(address.clone());
        assert!(!filter.check_filter(&item));
    }
}