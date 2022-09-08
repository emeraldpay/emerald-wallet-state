use std::str::FromStr;
use bitcoin::util::bip32::ExtendedPubKey;
use protobuf::ProtobufEnum;
use uuid::Uuid;
use crate::access::pagination::{PageQuery, PageResult};
use crate::errors::{InvalidValueError, StateError};
use crate::proto::addressbook::{Address, Address_AddressType, BookItem};
use crate::proto::transactions::BlockchainId;

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

    ///
    /// Validate the state of the Address Book Item to check that the data contains good values
    /// before storing it
    pub fn validate(&self) -> Result<(), InvalidValueError> {
        let blockchain = BlockchainId::from_i32(self.blockchain as i32)
            .ok_or(InvalidValueError::Name("blockchain".to_string()))?;
        match self.address.clone().into_option() {
            Some(address) => address.validate(blockchain),
            None => Err(InvalidValueError::NameMessage("address".to_string(), "Address is empty".to_string()))
        }
    }
}

impl Address {

    fn validate(&self, blockchain: BlockchainId) -> Result<(), InvalidValueError> {
        match self.get_field_type() {
            Address_AddressType::PLAIN => {
                match blockchain {
                    BlockchainId::CHAIN_BITCOIN | BlockchainId::CHAIN_TESTNET_BITCOIN => {
                        let _ = bitcoin::util::address::Address::from_str(self.address.as_str())
                            .map_err(|_| InvalidValueError::Other("Invalid address".to_string()))?;
                    },
                    // those are all ethereum blockchains
                    _ => {
                        let good_size = self.address.len() == 42;
                        let good_prefix = self.address.starts_with("0x");
                        if !good_size || !good_prefix {
                            return Err(InvalidValueError::Other("Invalid address".to_string()))
                        }
                        let is_hex = self.address[2..].chars().all(|c| c.is_ascii_hexdigit());
                        if !is_hex {
                            return Err(InvalidValueError::Other("Invalid address".to_string()))
                        }
                    }
                }
            }
            Address_AddressType::XPUB => {
                let _ = ExtendedPubKey::from_str(self.address.as_str())
                    .map_err(|_| InvalidValueError::Other("Not an XPub address".to_string()))?;
            }
        }
        Ok(())
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
    use crate::proto::addressbook::{BookItem as proto_BookItem, Address as proto_Address, Address_AddressType};

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

    #[test]
    fn accept_valid_ethereum_address() {
        let addresses = vec![
            "0xEdD91797204D3537fBaBDe0E0E42AaE99975f2Bb",
            "0x60bcd26c20586076eea2e7206e22bf5256e76a20",
            "0x000000000D71b31F9C460f26C45589EC91551969"
        ];

        for value in addresses {
            let mut item = proto_BookItem::new();
            item.id = "989d7648-13e3-4cb9-acfb-85464f063b34".to_string();
            item.blockchain = 101;
            let mut address = proto_Address::new();
            address.set_address(value.to_string());
            item.set_address(address.clone());
            assert!(item.validate().is_ok());
        }
    }

    #[test]
    fn deny_invalid_ethereum_address() {
        let addresses = vec![
            "0xEdD91797204D3537fBaBDe0E0E42AaE99975f2",
            "60bcd26c20586076eea2e7206e22bf5256e76a20",
            "0x000000000D71b31F9C460f26C45589EC9HELLO!!!"
        ];

        for value in addresses {
            let mut item = proto_BookItem::new();
            item.id = "989d7648-13e3-4cb9-acfb-85464f063b34".to_string();
            item.blockchain = 101;
            let mut address = proto_Address::new();
            address.set_address(value.to_string());
            item.set_address(address.clone());
            assert!(item.validate().is_err());
        }
    }

    #[test]
    fn accept_valid_bitcoin_address() {
        let addresses = vec![
            "18cBEMRxXHqzWWCxZNtU91F5sbUNKhL5PX",
            "bc1qemjjwfcq7vn7cn5lzsmy42d8fxk5ftkfrqtzzf",
            "bc1qt8lsk53uwckq06w7fea9uf0w4q6sp9p5m9s0m5",
            "36RJWEeCbitVUweiteec5BLkNjRjHgS7ES",
            "bc1qnsf32qwptm6mv9vwz3n6shs3j4dp4a8ale66qezmcp8exndczsasz0xx9y",
            "35iMHbUZeTssxBodiHwEEkb32jpBfVueEL"
        ];

        for value in addresses {
            let mut item = proto_BookItem::new();
            item.id = "989d7648-13e3-4cb9-acfb-85464f063b34".to_string();
            item.blockchain = 1;
            let mut address = proto_Address::new();
            address.set_address(value.to_string());
            item.set_address(address.clone());
            assert!(item.validate().is_ok());
        }
    }

    #[test]
    fn deny_invalid_bitcoin_address() {
        let addresses = vec![
            "18cBEMRxXHqzWWCxZNtU",
            "bc1qemjjwfcq7vn7cn5lzsmy4",
            "36RJWEeCbitVUweiteec5BLkNjRjHgS7ES!!!!!",
            "35iMHbUZeTssxBodiHwEEkb32jpBfV"
        ];

        for value in addresses {
            let mut item = proto_BookItem::new();
            item.id = "989d7648-13e3-4cb9-acfb-85464f063b34".to_string();
            item.blockchain = 1;
            let mut address = proto_Address::new();
            address.set_address(value.to_string());
            item.set_address(address.clone());
            assert!(item.validate().is_err());
        }
    }

    #[test]
    fn accept_valid_bitcoin_testnet_address() {
        let addresses = vec![
            "tb1qccr9f2fjfjj6ur72fljeug6x0guawuupcr234d",
            "tb1qxezg5rn0rqv40utm7v597dw3mp330umv7qpc02",
            "mzFUtQHL7PDj4ZrqpgQTUPWD178Rmqf2JJ",
            "2N4UNaRa9FQFGJsnN9Ybj6n7ASEZDAGovUa",
        ];

        for value in addresses {
            let mut item = proto_BookItem::new();
            item.id = "989d7648-13e3-4cb9-acfb-85464f063b34".to_string();
            item.blockchain = 1;
            let mut address = proto_Address::new();
            address.set_address(value.to_string());
            item.set_address(address.clone());
            assert!(item.validate().is_ok());
        }
    }

    #[test]
    fn accept_valid_bitcoin_xpub() {
        let addresses = vec![
            "xpub6EdMmyBKs9b1S54aHP13QGJRrpKzrnKUJnzLho64zSv5ekwGKA9dysTS6eTiypMMe8UbrFuZHo2hKB5MhWhEfGxAEzWv2tGUkPFnkvXLWWC",
        ];

        for value in addresses {
            let mut item = proto_BookItem::new();
            item.id = "989d7648-13e3-4cb9-acfb-85464f063b34".to_string();
            item.blockchain = 1;
            let mut address = proto_Address::new();
            address.set_address(value.to_string());
            address.set_field_type(Address_AddressType::XPUB);
            item.set_address(address.clone());
            assert!(item.validate().is_ok());
        }
    }

    #[test]
    fn deny_valid_ethereum_address_for_bitcoin() {
        let addresses = vec![
            "0xEdD91797204D3537fBaBDe0E0E42AaE99975f2Bb",
        ];

        for value in addresses {
            let mut item = proto_BookItem::new();
            item.id = "989d7648-13e3-4cb9-acfb-85464f063b34".to_string();
            item.blockchain = 1;
            let mut address = proto_Address::new();
            address.set_address(value.to_string());
            item.set_address(address.clone());
            assert!(item.validate().is_err());
        }
    }

    #[test]
    fn deny_valid_bitcoin_address_for_ethereum() {
        let addresses = vec![
            "bc1qemjjwfcq7vn7cn5lzsmy42d8fxk5ftkfrqtzzf",
        ];

        for value in addresses {
            let mut item = proto_BookItem::new();
            item.id = "989d7648-13e3-4cb9-acfb-85464f063b34".to_string();
            item.blockchain = 101;
            let mut address = proto_Address::new();
            address.set_address(value.to_string());
            item.set_address(address.clone());
            assert!(item.validate().is_err());
        }
    }

}