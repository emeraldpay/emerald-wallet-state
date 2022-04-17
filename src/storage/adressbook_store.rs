use std::collections::HashSet;
use std::ops::Deref;
use std::sync::Arc;
use protobuf::Message;
use sled::{Batch, Db};
use uuid::Uuid;
use chrono::{DateTime, Utc};
use crate::access::addressbook::{AddressBook, Filter};
use crate::access::pagination::{PageQuery, PageResult};
use crate::errors::StateError;
use crate::proto::addressbook::{BookItem as proto_BookItem};
use crate::storage::indexing::{IndexConvert, IndexedValue, IndexEncoding, Indexing, QueryRanges};

const PREFIX_KEY: &'static str = "addrbook:";
const PREFIX_IDX: &'static str = "idx:addrbook:";

enum IndexType {
    // `<LABEL>/<TIMESTAMP>`
    ByLabel(String, u64),
    // `<ADDR>/<TIMESTAMP>`
    ByAddress(String, u64),
    // `/<TIMESTAMP>`
    Everything(u64),
    // `/<LABEL-OR-ADDR>`
    ByAnyString(String)
}

impl IndexType {
    fn get_prefix(&self) -> usize {
        match self {
            IndexType::Everything(_) => 1,
            IndexType::ByLabel(_, _) => 2,
            IndexType::ByAddress(_, _) => 3,
            IndexType::ByAnyString(_) => 4,
        }
    }
}

impl IndexEncoding for IndexType {
    fn get_index_key(&self) -> String {
        match self {
            IndexType::ByLabel(label, ts) => format!("{}:{:}/{:}/{:}", PREFIX_IDX, self.get_prefix(), label, IndexConvert::get_desc_timestamp(*ts)),
            IndexType::ByAddress(addr, ts) => format!("{}:{:}/{:}/{:}", PREFIX_IDX, self.get_prefix(), addr, IndexConvert::get_desc_timestamp(*ts)),
            IndexType::Everything(ts) => format!("{}:{:}/{:}", PREFIX_IDX, self.get_prefix(), IndexConvert::get_desc_timestamp(*ts)),
            IndexType::ByAnyString(s) => format!("{}:{:}/{:}", PREFIX_IDX, self.get_prefix(), s),
        }
    }
}

impl QueryRanges for Filter {
    fn get_index_bounds(&self) -> (String, String) {
        // TODO use specific filter when available
        let start = IndexType::ByAnyString("0".to_string()).get_index_key();
        let end = IndexType::ByAnyString("Z".to_string()).get_index_key();
        (start, end)
    }
}

impl IndexedValue<IndexType> for proto_BookItem {
    fn get_index(&self) -> Vec<IndexType> {
        let mut keys: Vec<IndexType> = Vec::new();
        let ts = self.create_timestamp;

        keys.push(IndexType::Everything(ts));

        let mut any_string: Option<IndexType> = None;

        let label = self.get_label().trim();
        if !label.is_empty() {
            keys.push(IndexType::ByLabel(label.to_lowercase().to_string(), ts));
            if any_string.is_none() {
                any_string = Some(IndexType::ByAnyString(label.to_lowercase().to_string()));
            }
        }

        let address = &self.get_address().address.trim();
        if !address.is_empty() {
            keys.push(IndexType::ByAddress(address.to_lowercase().to_string(), ts));
            if any_string.is_none() {
                any_string = Some(IndexType::ByAnyString(address.to_lowercase().to_string()));
            }
        }

        if any_string.is_none() {
            any_string = Some(IndexType::ByAnyString(format!("{:#013}", ts)));
        }

        if let Some(any_strgin_idx) = any_string {
            keys.push(any_strgin_idx)
        }

        keys
    }
}

pub struct AddressBookAccess {
    pub(crate) db: Arc<Db>,
}

impl AddressBookAccess {
    fn get_key(id: Uuid) -> String {
        format!("{}{}", PREFIX_KEY, id.to_string())
    }

    fn extract_id(key: String) -> Result<Uuid, StateError> {
        if !key.starts_with(PREFIX_KEY) {
            return Err(StateError::InvalidId)
        }
        let id = key.get((PREFIX_KEY.len())..);
        if id.is_none() {
            return Err(StateError::InvalidId)
        }
        Uuid::parse_str(id.unwrap()).map_err(|_| StateError::InvalidId)
    }

    fn get_item(&self, id: Uuid) -> Option<proto_BookItem> {
        match self.db.get(AddressBookAccess::get_key(id)) {
            Ok(data) => {
                match data {
                    Some(b) => proto_BookItem::parse_from_bytes(b.deref()).ok(),
                    None => None
                }
            }
            Err(_) => None
        }
    }
}

impl AddressBook for AddressBookAccess {

    fn add(&self, items: Vec<proto_BookItem>) -> Result<Vec<Uuid>, StateError> {
        let mut batch = Batch::default();
        let mut ids = Vec::new();
        for item_source in items {
            // reuse existing id, or create a new id
            let mut item = if Uuid::parse_str(item_source.get_id()).is_ok() {
                item_source
            } else {
                let mut copy = item_source.clone();
                copy.id = Uuid::new_v4().to_string();
                copy
            };

            // it it's just a newly created record fill it with creation/update timestamp
            let now = Utc::now().naive_utc().timestamp_millis() as u64;
            if item.create_timestamp == 0 {
                item.create_timestamp = now;
            }
            if item.update_timestamp == 0 {
                item.update_timestamp = now;
            }

            let id = Uuid::parse_str(item.get_id()).unwrap();
            if let Ok(item_bytes) = item.write_to_bytes() {
                ids.push(id);
                let item_key = AddressBookAccess::get_key(id);
                let indexes: Vec<String> = item.get_index_keys();
                Indexing::add_backrefs(&indexes, item_key.clone(), &mut batch)?;
                for idx in indexes {
                    batch.insert(idx.as_bytes(), item_key.as_bytes());
                }
                batch.insert(item_key.as_bytes(), item_bytes);
            }
        }
        self.db.apply_batch(batch)
            .map_err(|e| StateError::from(e))
            .map(|_| ids)
    }

    fn remove(&self, id: Uuid) -> Result<(), StateError> {
        let mut batch = Batch::default();
        let item_key = AddressBookAccess::get_key(id);
        batch.remove(item_key.as_bytes());
        Indexing::remove_backref(item_key, self.db.clone(), &mut batch)?;
        self.db.apply_batch(batch)
            .map_err(|e| StateError::from(e))
    }

    fn query(&self, filter: Filter, page: PageQuery) -> Result<PageResult<proto_BookItem>, StateError> {
        let bounds = filter.get_index_bounds();
        let mut processed = HashSet::new();
        let mut iter = self.db
            .range(bounds.0..bounds.1);
        let mut done = false;

        let mut results = Vec::new();

        while !done {
            let next = iter.next();
            match next {
                Some(x) => match x {
                    Ok(v) => {
                        let itemkey = v.1.to_vec();
                        let itemkey = AddressBookAccess::extract_id(String::from_utf8(itemkey).unwrap())?;
                        let unprocessed = processed.insert(itemkey.clone());
                        if unprocessed {
                            if let Some(item) = self.get_item(itemkey) {
                                if filter.check_filter(&item) {
                                    results.push(item);
                                    if results.len() >= page.limit {
                                        done = true
                                    }
                                }
                            }
                        }
                    }
                    Err(_) => {}
                },
                None => done = true
            }
        }

        let result = PageResult {
            values: results,
            cursor: None,
        };

        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use tempdir::TempDir;
    use uuid::Uuid;
    use crate::access::addressbook::{AddressBook, Filter};
    use crate::access::pagination::PageQuery;
    use crate::storage::sled_access::SledStorage;
    use crate::proto::addressbook::{BookItem as proto_BookItem, Address as proto_Address};

    #[test]
    fn create_and_find() {
        let tmp_dir = TempDir::new("test-addressbook").unwrap();
        let access = SledStorage::open(tmp_dir.path().to_path_buf()).unwrap();
        let store = access.get_addressbook();

        let mut item = proto_BookItem::new();
        item.create_timestamp = 1_647_313_850_992;
        item.blockchain = 101;
        let mut address = proto_Address::new();
        address.address = "0xEdD91797204D3537fBaBDe0E0E42AaE99975f2Bb".to_string();
        item.set_address(address);

        let mut exp = item.clone();

        let results = store.add(vec![item.clone()]).expect("not saved");
        assert_eq!(results.len(), 1);
        exp.id = results[0].to_string();


        let results = store.query(Filter::default(), PageQuery::default()).expect("queried");
        assert_eq!(results.values.len(), 1);
        let mut result = results.values.get(0).unwrap().clone();
        result.update_timestamp = 0;
        assert_eq!(result, exp);
        assert!(results.cursor.is_none());
    }

    #[test]
    fn create_existing_and_find() {
        let tmp_dir = TempDir::new("test-addressbook").unwrap();
        let access = SledStorage::open(tmp_dir.path().to_path_buf()).unwrap();
        let store = access.get_addressbook();

        let mut item = proto_BookItem::new();
        item.id = "989d7648-13e3-4cb9-acfb-85464f063b34".to_string();
        item.create_timestamp = 1_647_313_850_992;
        item.blockchain = 101;
        let mut address = proto_Address::new();
        address.address = "0xEdD91797204D3537fBaBDe0E0E42AaE99975f2Bb".to_string();
        item.set_address(address);

        let exp = item.clone();

        let results = store.add(vec![item.clone()]).expect("not saved");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0], Uuid::parse_str("989d7648-13e3-4cb9-acfb-85464f063b34").unwrap());


        let results = store.query(Filter::default(), PageQuery::default()).expect("queried");
        assert_eq!(results.values.len(), 1);
        let mut result = results.values.get(0).unwrap().clone();
        result.update_timestamp = 0;

        assert_eq!(result, exp);
        assert!(results.cursor.is_none());
    }
}