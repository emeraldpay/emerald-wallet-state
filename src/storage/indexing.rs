use std::collections::HashSet;
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

pub trait QueryRanges {
    fn get_index_bounds(&self) -> (String, String);
}

pub struct  IndexConvert {
}

impl IndexConvert {
    /// Descending timestamp, because most of the UI supposed to show data from now backward it time
    pub fn get_desc_timestamp(ts: u64) -> String {
        // 1_647_313_850_992 - 13 characters
        format!("D{:#013}", 9_999_999_999_999 - ts)
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
}