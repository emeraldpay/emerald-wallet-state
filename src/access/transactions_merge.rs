//!
//! # Merge rules
//!
//! For a transaction we just get most fields from the newly proposed transaction, with exceptions for:
//! - get latest of `confirm_timestamp`
//! - keep `since_timestamp` if already set
//!
//! For _changes_ the process is a bit more complex. We distinguish two types of a change: transfer and fee.
//!
//! _Transfers_ are supposed to be updated (i.e. replaced), but we merge certain fields (see below)
//! if the change was already known. To do so we need to find mathing transfers, and we do that by amount.
//! All other changes (i.e., without matching amounts) considered as removed or added, depending on the side.
//!
//! Merging _transfers_:
//! - get fields from the proposed change
//! - but ensure that `wallet_id` and `entry_id` are not erased
//!
//! _Fees_ are replaced only if provided with update. I.e., if we have a fee already in the db we just
//! keep it as is. That's the case of bitcoin multi-user transaction, because we know our part of the fees
//! when we created the tx, and the following updates may not know our share.
//!
//!
use protobuf::RepeatedField;
use crate::proto::transactions::{Change, Change_ChangeType, Transaction};

impl Transaction {

    pub(crate) fn merge(self, update: Transaction) -> Transaction {
        let mut merged = update.clone();
        if update.confirm_timestamp < self.confirm_timestamp {
            merged.set_confirm_timestamp(self.confirm_timestamp);
        }
        if merged.since_timestamp == 0 {
            merged.set_since_timestamp(self.since_timestamp);
        }
        let changes = merge_changes(self.get_changes(), update.get_changes());
        merged.set_changes(RepeatedField::from_vec(changes));
        merged
    }
}

impl Change {
    pub(crate) fn is_similar_to(&self, another: &Change) -> bool {
        self.amount == another.amount && self.direction == another.direction && self.asset == another.asset && self.address == another.address
    }

    pub(crate) fn merge(self, update: Change) -> Change {
        let mut merged = update.clone();
        if update.wallet_id.is_empty() {
            merged.wallet_id = self.wallet_id;
            merged.entry_id = self.entry_id;
        }
        merged
    }
}

enum ChangeMerge {
    OLD(Change),
    NEW(Change),
    SAME(Change, Change)
}

impl ChangeMerge {
    fn merge(&self) -> Change {
        match self {
            ChangeMerge::OLD(v) => v.clone(),
            ChangeMerge::NEW(v) => v.clone(),
            ChangeMerge::SAME(a, b) => a.clone().merge(b.clone())
        }
    }
}

fn merge_changes(existing: &[Change], proposed: &[Change]) -> Vec<Change> {
    // get all transfers, including old, etc
    let transfers = merge_changes_transfer(
        only_change_type(existing, Change_ChangeType::TRANSFER),
        only_change_type(proposed, Change_ChangeType::TRANSFER)
    );

    // check if we have a proposed fees, otherwise just use the previous change for fee if it exist
    let proposed_fees = only_change_type(proposed, Change_ChangeType::FEE);
    let fees = if proposed_fees.is_empty() {
        only_change_type(existing, Change_ChangeType::FEE)
    } else {
        proposed_fees
    };

    let transfers: Vec<Change> = transfers.iter()
        .filter(|m| {
            // drop all changes that didn't come with the update
            match m { ChangeMerge::OLD(_) => false, _ => true }
        })
        .map(|m| m.merge())
        // don't forget about the fees
        .chain(fees)
        .collect();

    transfers
}

fn only_change_type(changes: &[Change], change_type: Change_ChangeType) -> Vec<Change> {
    changes.iter()
        .filter(|c| c.change_type == change_type )
        .map(|c| c.clone())
        .collect()
}

fn merge_changes_transfer(left: Vec<Change>, right: Vec<Change>) -> Vec<ChangeMerge> {
    let mut right_pool = right;
    let mut result = vec![];

    // first check if we have associated changes with the new proposal
    for x in left {
        let similar = right_pool.iter()
            .position(|a| x.is_similar_to(a));
        match similar {
            Some(a) => {
                // we found two similar changes, will merge them later
                let a = right_pool.remove(a);
                result.push(ChangeMerge::SAME(x, a.clone()));
            },
            None => {
                // no associated update, so assume that the existing change is "old" and may be dropped later
                result.push(ChangeMerge::OLD(x))
            }
        }
    }
    // add whatever left in the proposal as "new" changes, will be stored as is
    for y in right_pool {
        result.push(ChangeMerge::NEW(y))
    }
    result
}

#[cfg(test)]
mod tests {
    use crate::proto::transactions::{BlockchainId, Change, Change_ChangeType, Direction, Transaction};

    #[test]
    fn merge_same_transaction() {
        let mut tx = Transaction::new();
        tx.blockchain = BlockchainId::CHAIN_ETHEREUM;
        tx.since_timestamp = 1_647_313_850_992;
        let mut change1 = Change::new();
        change1.wallet_id = "72279ede-44c4-4951-925b-f51a7b9e929a".to_string();
        change1.entry_id = 0;
        change1.address = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_string();
        change1.amount = "100000000".to_string();
        change1.direction = Direction::SEND;
        change1.change_type = Change_ChangeType::TRANSFER;
        tx.changes.push(change1);

        let merged = tx.clone().merge(tx.clone());

        assert_eq!(tx, merged);
    }

    #[test]
    fn keeps_wallet_id() {
        let mut tx = Transaction::new();
        tx.blockchain = BlockchainId::CHAIN_ETHEREUM;
        tx.since_timestamp = 1_647_313_850_992;
        let mut change1 = Change::new();
        change1.wallet_id = "72279ede-44c4-4951-925b-f51a7b9e929a".to_string();
        change1.entry_id = 1;
        change1.address = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_string();
        change1.amount = "100000000".to_string();
        change1.direction = Direction::SEND;
        change1.change_type = Change_ChangeType::TRANSFER;

        let mut change1_copy = change1.clone();

        tx.changes.push(change1);

        let mut tx_new = tx.clone();
        change1_copy.clear_entry_id();
        change1_copy.clear_wallet_id();
        tx_new.changes.clear();
        tx_new.changes.push(change1_copy);

        let merged = tx.clone().merge(tx_new);

        assert_eq!(tx, merged);
    }

    #[test]
    fn updates_wallet_id() {
        let mut tx = Transaction::new();
        tx.blockchain = BlockchainId::CHAIN_ETHEREUM;
        tx.since_timestamp = 1_647_313_850_992;
        let mut change1 = Change::new();
        change1.address = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_string();
        change1.amount = "100000000".to_string();
        change1.direction = Direction::SEND;
        change1.change_type = Change_ChangeType::TRANSFER;

        let mut change1_copy = change1.clone();

        tx.changes.push(change1);

        let mut tx_new = tx.clone();
        change1_copy.wallet_id = "72279ede-44c4-4951-925b-f51a7b9e929a".to_string();
        change1_copy.entry_id = 5;
        tx_new.changes.clear();
        tx_new.changes.push(change1_copy);

        let merged = tx.clone().merge(tx_new.clone());

        assert_eq!(tx_new, merged);
    }

    #[test]
    fn replace_same_change() {
        let mut change1 = Change::new();
        change1.address = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_string();
        change1.amount = "100000000".to_string();
        change1.direction = Direction::SEND;
        change1.change_type = Change_ChangeType::TRANSFER;
        change1.wallet_id = "72279ede-44c4-4951-925b-f51a7b9e929a".to_string();
        change1.entry_id = 5;

        let mut change2 = change1.clone();
        change2.amount = "100000015".to_string();

        let mut tx = Transaction::new();
        tx.blockchain = BlockchainId::CHAIN_ETHEREUM;
        tx.since_timestamp = 1_647_313_850_992;
        tx.changes.push(change1);

        let mut tx_new = tx.clone();
        tx_new.changes.clear();
        tx_new.changes.push(change2.clone());

        let merged = tx.clone().merge(tx_new.clone());

        assert_eq!(merged.changes.len(), 1);
        assert_eq!(merged.changes.get(0).unwrap(), &change2);
    }

    #[test]
    fn replace_all_changes() {
        let mut change1 = Change::new();
        change1.address = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_string();
        change1.amount = "100000000".to_string();
        change1.direction = Direction::SEND;
        change1.change_type = Change_ChangeType::TRANSFER;
        change1.wallet_id = "72279ede-44c4-4951-925b-f51a7b9e929a".to_string();
        change1.entry_id = 5;

        let mut change2 = change1.clone();
        change2.amount = "100000015".to_string();

        let mut change3 = Change::new();
        change3.address = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_string();
        change3.amount = "500000".to_string();
        change3.direction = Direction::SEND;
        change3.change_type = Change_ChangeType::TRANSFER;
        change3.wallet_id = "72279ede-44c4-4951-925b-f51a7b9e929a".to_string();
        change3.entry_id = 5;

        let mut tx = Transaction::new();
        tx.blockchain = BlockchainId::CHAIN_ETHEREUM;
        tx.since_timestamp = 1_647_313_850_992;
        tx.changes.push(change1);

        let mut tx_new = tx.clone();
        tx_new.changes.clear();
        tx_new.changes.push(change2.clone());
        tx_new.changes.push(change3.clone());

        let merged = tx.clone().merge(tx_new.clone());

        assert_eq!(merged.changes.len(), 2);
        assert_eq!(merged.changes.get(0).unwrap(), &change2);
        assert_eq!(merged.changes.get(1).unwrap(), &change3);
    }

    #[test]
    fn keep_fees() {
        let mut change1 = Change::new();
        change1.address = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_string();
        change1.amount = "100000000".to_string();
        change1.direction = Direction::SEND;
        change1.change_type = Change_ChangeType::TRANSFER;
        change1.wallet_id = "72279ede-44c4-4951-925b-f51a7b9e929a".to_string();
        change1.entry_id = 5;

        let mut change_fee1 = Change::new();
        change_fee1.address = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_string();
        change_fee1.amount = "300".to_string();
        change_fee1.direction = Direction::SEND;
        change_fee1.change_type = Change_ChangeType::FEE;
        change_fee1.wallet_id = "72279ede-44c4-4951-925b-f51a7b9e929a".to_string();
        change_fee1.entry_id = 5;

        let mut change2 = change1.clone();
        change2.amount = "100000015".to_string();

        let mut change3 = Change::new();
        change3.address = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_string();
        change3.amount = "500000".to_string();
        change3.direction = Direction::SEND;
        change3.change_type = Change_ChangeType::TRANSFER;
        change3.wallet_id = "72279ede-44c4-4951-925b-f51a7b9e929a".to_string();
        change3.entry_id = 5;

        let mut tx = Transaction::new();
        tx.blockchain = BlockchainId::CHAIN_ETHEREUM;
        tx.since_timestamp = 1_647_313_850_992;
        tx.changes.push(change1.clone());
        tx.changes.push(change_fee1.clone());

        let mut tx_new = tx.clone();
        tx_new.changes.clear();
        tx_new.changes.push(change2.clone());
        tx_new.changes.push(change3.clone());

        let merged = tx.clone().merge(tx_new.clone());

        assert_eq!(merged.changes.len(), 3);
        assert_eq!(merged.changes.get(0).unwrap(), &change2);
        assert_eq!(merged.changes.get(1).unwrap(), &change3);
        assert_eq!(merged.changes.get(2).unwrap(), &change_fee1);
    }

    #[test]
    fn updates_fee_if_new_come() {
        let mut change1 = Change::new();
        change1.address = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_string();
        change1.amount = "100000000".to_string();
        change1.direction = Direction::SEND;
        change1.change_type = Change_ChangeType::TRANSFER;
        change1.wallet_id = "72279ede-44c4-4951-925b-f51a7b9e929a".to_string();
        change1.entry_id = 5;

        let mut change_fee1 = Change::new();
        change_fee1.address = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_string();
        change_fee1.amount = "300".to_string();
        change_fee1.direction = Direction::SEND;
        change_fee1.change_type = Change_ChangeType::FEE;
        change_fee1.wallet_id = "72279ede-44c4-4951-925b-f51a7b9e929a".to_string();
        change_fee1.entry_id = 5;

        let mut change2 = change1.clone();
        change2.amount = "100000015".to_string();

        let mut change3 = Change::new();
        change3.address = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_string();
        change3.amount = "500000".to_string();
        change3.direction = Direction::SEND;
        change3.change_type = Change_ChangeType::TRANSFER;
        change3.wallet_id = "72279ede-44c4-4951-925b-f51a7b9e929a".to_string();
        change3.entry_id = 5;

        let mut change_fee4 = Change::new();
        change_fee4.address = "0xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48".to_string();
        change_fee4.amount = "381".to_string();
        change_fee4.direction = Direction::SEND;
        change_fee4.change_type = Change_ChangeType::FEE;
        change_fee4.wallet_id = "72279ede-44c4-4951-925b-f51a7b9e929a".to_string();
        change_fee4.entry_id = 5;

        let mut tx = Transaction::new();
        tx.blockchain = BlockchainId::CHAIN_ETHEREUM;
        tx.since_timestamp = 1_647_313_850_992;
        tx.changes.push(change1.clone());
        tx.changes.push(change_fee1.clone());

        let mut tx_new = tx.clone();
        tx_new.changes.clear();
        tx_new.changes.push(change2.clone());
        tx_new.changes.push(change3.clone());
        tx_new.changes.push(change_fee4.clone());

        let merged = tx.clone().merge(tx_new.clone());

        assert_eq!(merged.changes.len(), 3);
        assert_eq!(merged.changes.get(0).unwrap(), &change2);
        assert_eq!(merged.changes.get(1).unwrap(), &change3);
        assert_eq!(merged.changes.get(2).unwrap(), &change_fee4);
    }
}