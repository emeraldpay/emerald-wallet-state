use std::collections::HashSet;
use std::ops::Bound;
use std::sync::Arc;
use chrono::Utc;
use protobuf::{Message, RepeatedField};
use sled::{Batch, Db};
use crate::errors::StateError;
use crate::proto::internal::{Indexes as proto_Indexes};

const IDX_BACKREF: &'static str = "idx_back:";

pub(crate) struct Indexing {}

impl Indexing {

    ///
    /// Keeps track of all indexes per item, which can be user to clear them later.
    pub fn add_backrefs(indexes: &Vec<String>, target_key: String, batch: &mut Batch) -> Result<(), StateError>{
        let mut index_backref = proto_Indexes::new();
        index_backref.set_keys(RepeatedField::from_vec(indexes.clone()));
        let value = index_backref.write_to_bytes()?;
        batch.insert(
            // we can expect multiple updates of the `target_key` with different indexes, so we track all versions of them by timestamping each
            format!("{}{}/{}", IDX_BACKREF, target_key, Utc::now().naive_utc().timestamp_millis() as u64).as_bytes(),
            value.as_slice(),
        );
        Ok(())
    }

    ///
    /// Remove all indexes for the specified `target_key`
    pub fn remove_backref(target_key: String, db: Arc<Db>, batch: &mut Batch) -> Result<(), StateError> {
        let refs = db.scan_prefix(format!("{}{}/", IDX_BACKREF, target_key));
        let mut deleting = HashSet::new();
        for row in refs {
            let row = row.unwrap();
            let backref = row.1;
            // don't forget to remove the backref itself
            batch.remove(row.0);
            let backref = proto_Indexes::parse_from_bytes(backref.as_ref());
            if let Ok(m) = backref {
                for key in m.keys {
                    if deleting.insert(key.clone()) {
                        batch.remove(key.as_bytes())
                    }
                }
            }
        }
        Ok(())
    }
}


pub trait IndexedValue<T> where T: IndexEncoding + Sized + 'static {

    /// Get index keys for the storage, i.e. values used to index and query actual data.
    /// All returned values are mapped to the same item
    fn get_index(&self) -> Vec<T>;

    /// Indexes as strings
    fn get_index_keys(&self) -> Vec<String> {
        let mut result: Vec<String> = self.get_index()
            .iter()
            .map(|k| k.get_index_key())
            .collect();

        // we need to sort list to remove duplicates
        result.sort();
        result.dedup();
        result
    }
}

pub trait IndexEncoding {
    fn get_index_key(&self) -> String;
}

///
/// Defines the date required to query all possible entries under the trait
pub trait QueryRanges {
    ///
    /// Bounds of the indexes. Note that it query for _indexes_, not actual entries
    fn get_index_bounds(&self) -> (Bound<String>, Bound<String>);
}

pub struct  IndexConvert {
}

#[allow(dead_code)]
impl IndexConvert {
    /// Descending timestamp, because most of the UI supposed to show data from now backward it time
    pub fn get_desc_timestamp(ts: u64) -> String {
        // 1_647_313_850_992 - 13 characters
        format!("D{:#013}", 9_999_999_999_999 - ts)
    }

    pub fn get_asc_number(v: u64) -> String {
        // 20 characters
        format!("A{:#020}", v)
    }

    pub fn get_desc_number(v: u64) -> String {
        // 20 characters (2^64 == 18_446_744_073_709_551_616)
        format!("A{:#020}", u64::MAX - v)
    }

    /// Index when FALSE should go before TRUE
    pub fn get_bool_ft(v: &bool) -> String {
        if *v {"F1".to_string()} else {"F0".to_string()}
    }

    /// Index when TRUE should go before FALSE
    pub fn get_bool_tf(v: &bool) -> String {
        if *v {"T0".to_string()} else {"T1".to_string()}
    }

    ///
    /// To index a tx id in ASC order by taking only first 64 bit of the value.
    /// The id is threaten as a hex value with an optional `0x` prefix.
    pub fn txid_as_pos(tx_id: String) -> u64 {
        hex::decode(tx_id.trim_start_matches("0x"))
            .map(|txid| {
                let bytes: [u8; 8] = if txid.len() >= 8 {
                    txid.as_slice()[0..8].try_into().unwrap()
                } else {
                    let mut buf = [0u8; 8];
                    buf[(8-txid.len())..8].clone_from_slice(txid.as_slice());
                    buf
                };
                u64::from_be_bytes(bytes)
            }).ok().unwrap_or(0)
    }

    pub fn from_encodable<T>(keys: Vec<T>) -> Vec<String>
        where T: IndexEncoding + Sized + 'static {
        let mut result: Vec<String> = keys
            .iter()
            .map(|k| k.get_index_key())
            .collect();

        result.sort();
        result.dedup();
        result
    }
}

#[cfg(test)]
mod tests {
    use std::cmp::Ordering;
    use super::IndexConvert;

    #[test]
    fn format_ts() {
        let act = IndexConvert::get_desc_timestamp(1_647_313_850_992);
        assert_eq!(act, "D8352686149007");
    }

    #[test]
    fn format_zero_ts() {
        let act = IndexConvert::get_desc_timestamp(0);
        assert_eq!(act, "D9999999999999");
    }

    #[test]
    fn order_ts_desc() {
        assert_eq!(IndexConvert::get_desc_timestamp(0).cmp(&IndexConvert::get_desc_timestamp(1000)), Ordering::Greater);
    }

    #[test]
    fn format_number_desc() {
        assert_eq!(IndexConvert::get_desc_number(1_647_313_850_992), "A18446742426395700623");
        assert_eq!(IndexConvert::get_desc_number(0),                 "A18446744073709551615");
        assert_eq!(IndexConvert::get_desc_number(u64::MAX),          "A00000000000000000000");
    }

    #[test]
    fn order_number_desc() {
        // DESC -> big numbers come small
        assert_eq!(IndexConvert::get_desc_number(1000).cmp(&IndexConvert::get_desc_number(500)),     Ordering::Less);
        assert_eq!(IndexConvert::get_desc_number(1000).cmp(&IndexConvert::get_desc_number(999)),     Ordering::Less);
        assert_eq!(IndexConvert::get_desc_number(1000).cmp(&IndexConvert::get_desc_number(1001)),    Ordering::Greater);
        assert_eq!(IndexConvert::get_desc_number(1000).cmp(&IndexConvert::get_desc_number(0)),       Ordering::Less);
        assert_eq!(IndexConvert::get_desc_number(1000).cmp(&IndexConvert::get_desc_number(10_000)),  Ordering::Greater);
    }

    #[test]
    fn format_bool_tf() {
        assert_eq!(IndexConvert::get_bool_tf(&true),  "T0");
        assert_eq!(IndexConvert::get_bool_tf(&false), "T1");
    }

    #[test]
    fn format_bool_ft() {
        assert_eq!(IndexConvert::get_bool_ft(&true),  "F1");
        assert_eq!(IndexConvert::get_bool_ft(&false), "F0");
    }

    #[test]
    fn order_bool_tf() {
        // TRUE comes before FALSE
        assert_eq!(IndexConvert::get_bool_tf(&true).cmp(&IndexConvert::get_bool_tf(&false)),     Ordering::Less);
        assert_eq!(IndexConvert::get_bool_tf(&false).cmp(&IndexConvert::get_bool_tf(&true)),     Ordering::Greater);
        assert_eq!(IndexConvert::get_bool_tf(&false).cmp(&IndexConvert::get_bool_tf(&false)),     Ordering::Equal);
        assert_eq!(IndexConvert::get_bool_tf(&true).cmp(&IndexConvert::get_bool_tf(&true)),     Ordering::Equal);
    }

    #[test]
    fn order_bool_ft() {
        // FALSE comes before TRUE
        assert_eq!(IndexConvert::get_bool_ft(&true).cmp(&IndexConvert::get_bool_ft(&false)),     Ordering::Greater);
        assert_eq!(IndexConvert::get_bool_ft(&false).cmp(&IndexConvert::get_bool_ft(&true)),     Ordering::Less);
        assert_eq!(IndexConvert::get_bool_ft(&false).cmp(&IndexConvert::get_bool_ft(&false)),     Ordering::Equal);
        assert_eq!(IndexConvert::get_bool_ft(&true).cmp(&IndexConvert::get_bool_ft(&true)),     Ordering::Equal);
    }

    #[test]
    fn format_short_tx_id() {
        assert_eq!(IndexConvert::txid_as_pos("".to_string()), 0x0);
        assert_eq!(IndexConvert::txid_as_pos("000".to_string()), 0x0);
        assert_eq!(IndexConvert::txid_as_pos("ff".to_string()), 0xff);
    }

    #[test]
    fn format_invalid_tx_id() {
        assert_eq!(IndexConvert::txid_as_pos("NONE".to_string()), 0x0);
    }

    #[test]
    fn format_ethereum_tx() {
        assert_eq!(IndexConvert::txid_as_pos("0x275a4b69b11068633e5729427d1da63368c2a6ed6fbaafde522f1e1eb10e2d49".to_string()), 0x275a4b69b1106863);
    }

    #[test]
    fn format_bitcoin_tx() {
        assert_eq!(IndexConvert::txid_as_pos("8f76ace471e8553eef24f10f6286838a2271e5505bb934a2af8cd37aae3a3eb1".to_string()), 0x8f76ace471e8553e);
    }
}