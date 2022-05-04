use chrono::{DateTime, Utc};
use protobuf::ProtobufEnum;
use uuid::Uuid;
use crate::access::pagination::{PageQuery, PageResult};
use crate::errors::StateError;
use crate::proto::transactions::Transaction;

#[derive(Debug, Clone)]
/// Reference to a wallet or its part
pub enum WalletRef {
    /// Whole wallet with all its entries
    WholeWallet(Uuid),
    /// Specific entry at the position
    SelectedEntry(Uuid, u32),
}

#[derive(Debug, Clone)]
/// Reference to an address
pub enum AddressRef {
    /// A single address, represented as a string. Note that it's case sensitive, and for Ethereum
    /// addresses it supposed to be in lowercase
    SingleAddress(String),
    /// Reference to a series of addresses on the Xpub (first param), starting at X with window N
    Xpub(String, u32, u32),
}

#[derive(Debug, Clone)]
/// Transactions Query Filter to select which transactions are accepted.
/// It's _AND_ type of filter between groups, i.e. all of the non-empty criteria are required, but
/// each of the group may have different acceptance logic.
/// If it's empty - all transactions are accepted.
pub struct Filter {
    /// Require the specified wallet
    pub wallet: Option<WalletRef>,
    /// Require any of the specified addresses
    pub addresses: Option<Vec<AddressRef>>,
    /// Require any of the specified blockchains
    pub blockchains: Option<Vec<u32>>,
    /// require a transaction known or confirmed after the specified moment
    pub after: Option<DateTime<Utc>>,
    /// require a transaction known or confirmed before the specified moment
    pub before: Option<DateTime<Utc>>,
}

impl Default for Filter {
    fn default() -> Self {
        Filter {
            wallet: None,
            addresses: None,
            blockchains: None,
            after: None,
            before: None,
        }
    }
}

///
/// A reference to an external _cursor_ used to fetch updates for an address
#[derive(Debug, Clone)]
pub struct RemoteCursor {
    /// Cursor value
    pub value: String,
    /// When the cursor was provided
    pub since: DateTime<Utc>,
}

impl Filter {
    /// Checks the filter against the transaction.
    /// Returns `true` if the transaction fits the criteria
    pub fn check_filter(&self, t: &Transaction) -> bool {
        let tbid: u32 = t.blockchain.value() as u32;
        let blockchains_ok = if let Some(blockchains) = &self.blockchains {
            blockchains.iter().any(|b| tbid == *b)
        } else { true };

        if !blockchains_ok {
            return false;
        }

        let after_ok = match &self.after.map(|ts| ts.timestamp_millis() as u64) {
            Some(ts) => (t.since_timestamp != 0 && t.since_timestamp >= *ts) || (t.confirm_timestamp != 0 && t.confirm_timestamp >= *ts),
            None => true
        };
        let before_ok = match &self.before.map(|ts| ts.timestamp_millis() as u64) {
            Some(ts) => (t.since_timestamp != 0 && t.since_timestamp <= *ts) || (t.confirm_timestamp != 0 && t.confirm_timestamp <= *ts),
            None => true
        };
        let time_ok = after_ok && before_ok;
        if !time_ok {
            return false;
        }

        let wallet_ok = if let Some(w) = &self.wallet {
            match w {
                WalletRef::WholeWallet(uuid) => {
                    t.get_changes().iter().any(|c| c.wallet_id == uuid.to_string())
                }
                WalletRef::SelectedEntry(uuid, index) => {
                    t.get_changes().iter().any(|c| c.wallet_id == uuid.to_string() && c.entry_id == *index)
                }
            }
        } else { true };

        let address_ok = if let Some(addresses) = &self.addresses {
            addresses.iter().any(|a|
                match a {
                    AddressRef::SingleAddress(addr) => t.get_changes().iter().any(|c| c.address.eq(addr)),
                    AddressRef::Xpub(_, _, _) => todo!()
                }
            )
        } else { true };

        wallet_ok && address_ok
    }
}

pub trait Transactions {
    ///
    /// Find transactions given filter
    fn query(&self, filter: Filter, page: PageQuery) -> Result<PageResult<Transaction>, StateError>;

    fn get_tx(&self, blockchain: u32, txid: &str) -> Option<Transaction>;

    ///
    /// Update a new transactions. Update may be a new transactions or a new state to an existing
    /// Ex. initially a tx added with basic details only, just for future reference, and then updated when it changed
    fn submit(&self, transactions: Vec<Transaction>) -> Result<(), StateError>;

    ///
    /// Remove transaction from index
    fn forget(&self, blockchain: u32, tx_id: String) -> Result<(), StateError>;

    ///
    /// Get total count of transactions by given filter
    fn get_count(&self, filter: Filter) -> Result<usize, StateError>;

    ///
    /// Get current `cursor` for an `address`.
    fn get_cursor<S: AsRef<str>>(&self, address: S) -> Result<Option<RemoteCursor>, StateError>;

    ///
    /// Update `cursor` value for an `address`
    fn set_cursor<S: AsRef<str> + ToString>(&self, address: S, cursor: S) -> Result<(), StateError>;
}


#[cfg(test)]
mod tests {
    use std::str::FromStr;
    use protobuf::ProtobufEnum;
    use uuid::Uuid;
    use crate::access::transactions::{AddressRef, Filter, WalletRef};
    use crate::proto::transactions::{BlockchainId, Transaction as proto_Transaction, Change as proto_Change};

    #[test]
    fn empty_filter_accept_any() {
        let mut tx = proto_Transaction::new();
        tx.blockchain = BlockchainId::CHAIN_ETHEREUM;
        tx.since_timestamp = 1_647_313_850_992;
        let mut change1 = proto_Change::new();
        change1.wallet_id = "72279ede-44c4-4951-925b-f51a7b9e929a".to_string();
        change1.entry_id = 0;
        change1.address = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_string();
        tx.changes.push(change1);

        let filter = Filter::default();
        let ok = filter.check_filter(&tx);
        assert!(ok)
    }

    #[test]
    fn filter_by_blockchain() {
        let mut tx = proto_Transaction::new();
        tx.blockchain = BlockchainId::CHAIN_ETHEREUM;
        tx.since_timestamp = 1_647_313_850_992;
        let mut change1 = proto_Change::new();
        change1.wallet_id = "72279ede-44c4-4951-925b-f51a7b9e929a".to_string();
        change1.entry_id = 0;
        change1.address = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_string();
        tx.changes.push(change1);

        let filter = Filter {
            blockchains: Some(vec![BlockchainId::CHAIN_ETHEREUM.value() as u32]),
            ..Filter::default()
        };
        let ok = filter.check_filter(&tx);
        assert!(ok)
    }

    #[test]
    fn filter_by_blockchain_when_one_ok() {
        let mut tx = proto_Transaction::new();
        tx.blockchain = BlockchainId::CHAIN_ETHEREUM;
        tx.since_timestamp = 1_647_313_850_992;
        let mut change1 = proto_Change::new();
        change1.wallet_id = "72279ede-44c4-4951-925b-f51a7b9e929a".to_string();
        change1.entry_id = 0;
        change1.address = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_string();
        tx.changes.push(change1);

        let filter = Filter {
            blockchains: Some(vec![BlockchainId::CHAIN_ETHEREUM.value() as u32, BlockchainId::CHAIN_BITCOIN.value() as u32]),
            ..Filter::default()
        };
        let ok = filter.check_filter(&tx);
        assert!(ok)
    }

    #[test]
    fn filter_by_blockchain_when_none_ok() {
        let mut tx = proto_Transaction::new();
        tx.blockchain = BlockchainId::CHAIN_ETHEREUM;
        tx.since_timestamp = 1_647_313_850_992;
        let mut change1 = proto_Change::new();
        change1.wallet_id = "72279ede-44c4-4951-925b-f51a7b9e929a".to_string();
        change1.entry_id = 0;
        change1.address = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_string();
        tx.changes.push(change1);

        let filter = Filter {
            blockchains: Some(vec![BlockchainId::CHAIN_ETHEREUM_CLASSIC.value() as u32, BlockchainId::CHAIN_BITCOIN.value() as u32]),
            ..Filter::default()
        };
        let ok = filter.check_filter(&tx);
        assert!(!ok)
    }

    #[test]
    fn filter_by_wallet() {
        let mut tx = proto_Transaction::new();
        tx.blockchain = BlockchainId::CHAIN_ETHEREUM;
        tx.since_timestamp = 1_647_313_850_992;
        let mut change1 = proto_Change::new();
        change1.wallet_id = "72279ede-44c4-4951-925b-f51a7b9e929a".to_string();
        change1.entry_id = 0;
        change1.address = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_string();
        tx.changes.push(change1);

        let filter = Filter {
            wallet: Some(WalletRef::WholeWallet(Uuid::from_str("72279ede-44c4-4951-925b-f51a7b9e929a").unwrap())),
            ..Filter::default()
        };
        let ok = filter.check_filter(&tx);
        assert!(ok)
    }

    #[test]
    fn filter_by_wallet_different() {
        let mut tx = proto_Transaction::new();
        tx.blockchain = BlockchainId::CHAIN_ETHEREUM;
        tx.since_timestamp = 1_647_313_850_992;
        let mut change1 = proto_Change::new();
        change1.wallet_id = "72279ede-44c4-4951-925b-f51a7b9e929a".to_string();
        change1.entry_id = 0;
        change1.address = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_string();
        tx.changes.push(change1);

        let filter = Filter {
            wallet: Some(WalletRef::WholeWallet(Uuid::from_str("12279ede-44c4-4951-925b-f51a7b9e929a").unwrap())),
            ..Filter::default()
        };
        let ok = filter.check_filter(&tx);
        assert!(!ok)
    }

    #[test]
    fn filter_by_address() {
        let mut tx = proto_Transaction::new();
        tx.blockchain = BlockchainId::CHAIN_ETHEREUM;
        tx.since_timestamp = 1_647_313_850_992;
        let mut change1 = proto_Change::new();
        change1.wallet_id = "72279ede-44c4-4951-925b-f51a7b9e929a".to_string();
        change1.entry_id = 0;
        change1.address = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_string();
        tx.changes.push(change1);

        let filter = Filter {
            addresses: Some(vec![AddressRef::SingleAddress("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_string())]),
            ..Filter::default()
        };
        let ok = filter.check_filter(&tx);
        assert!(ok)
    }

    #[test]
    fn filter_by_address_different() {
        let mut tx = proto_Transaction::new();
        tx.blockchain = BlockchainId::CHAIN_ETHEREUM;
        tx.since_timestamp = 1_647_313_850_992;
        let mut change1 = proto_Change::new();
        change1.wallet_id = "72279ede-44c4-4951-925b-f51a7b9e929a".to_string();
        change1.entry_id = 0;
        change1.address = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_string();
        tx.changes.push(change1);

        let filter = Filter {
            addresses: Some(vec![AddressRef::SingleAddress("0x36c1d19d4a2e9eb0ce3606eb48a0b86991c6218b".to_string())]),
            ..Filter::default()
        };
        let ok = filter.check_filter(&tx);
        assert!(!ok)
    }

    #[test]
    fn filter_by_wallet_and_address() {
        let mut tx = proto_Transaction::new();
        tx.blockchain = BlockchainId::CHAIN_ETHEREUM;
        tx.since_timestamp = 1_647_313_850_992;
        let mut change1 = proto_Change::new();
        change1.wallet_id = "72279ede-44c4-4951-925b-f51a7b9e929a".to_string();
        change1.entry_id = 0;
        change1.address = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_string();
        tx.changes.push(change1);

        let filter = Filter {
            wallet: Some(WalletRef::WholeWallet(Uuid::from_str("72279ede-44c4-4951-925b-f51a7b9e929a").unwrap())),
            addresses: Some(vec![AddressRef::SingleAddress("0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_string())]),
            ..Filter::default()
        };
        let ok = filter.check_filter(&tx);
        assert!(ok)
    }

    #[test]
    fn filter_by_wallet_ok_but_address_different() {
        let mut tx = proto_Transaction::new();
        tx.blockchain = BlockchainId::CHAIN_ETHEREUM;
        tx.since_timestamp = 1_647_313_850_992;
        let mut change1 = proto_Change::new();
        change1.wallet_id = "72279ede-44c4-4951-925b-f51a7b9e929a".to_string();
        change1.entry_id = 0;
        change1.address = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_string();
        tx.changes.push(change1);

        let filter = Filter {
            wallet: Some(WalletRef::WholeWallet(Uuid::from_str("12279ede-44c4-4951-925b-f51a7b9e929a").unwrap())),
            addresses: Some(vec![AddressRef::SingleAddress("0x36c1d19d4a2e9eb0ce3606eb48a0b86991c6218b".to_string())]),
            ..Filter::default()
        };
        let ok = filter.check_filter(&tx);
        assert!(!ok)
    }
}