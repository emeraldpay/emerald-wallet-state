use std::collections::HashSet;
use std::ops::{Bound, Deref};
use std::sync::Arc;
use protobuf::Message;
use sled::{Batch, Db};
use uuid::Uuid;
use chrono::{Utc};
use crate::access::addressbook::{AddressBook, Filter};
use crate::access::pagination::{Cursor, PageQuery, PageResult};
use crate::errors::StateError;
use crate::proto::addressbook::{BookItem as proto_BookItem};
use crate::storage::indexing::{IndexConvert, IndexedValue, IndexEncoding, Indexing, QueryRanges};
use crate::storage::trigrams::Trigram;

const PREFIX_KEY: &'static str = "addrbook";
const PREFIX_IDX: &'static str = "idx:addrbook";

enum IndexType {
    // `<ADDR>/<TIMESTAMP>`
    ByAddress(String, u64),
    // `/<TIMESTAMP>`
    Everything(u64),
    // `/<TRIGRAM>/<TIMESTAMP>` timestamp is mostly used for uniquiness, but also gives a useful order
    ByTrigram(String, u64)
}

impl IndexType {
    fn get_prefix(&self) -> usize {
        match self {
            IndexType::Everything(_) => 1,
            IndexType::ByAddress(_, _) => 2,
            IndexType::ByTrigram(_, _) => 3,
        }
    }
}

impl IndexEncoding for IndexType {
    fn get_index_key(&self) -> String {
        match self {
            IndexType::ByAddress(addr, ts) => format!("{}:{:}/{:}/{:}", PREFIX_IDX, self.get_prefix(), addr, IndexConvert::get_desc_timestamp(*ts)),
            IndexType::Everything(ts) => format!("{}:{:}/{:}", PREFIX_IDX, self.get_prefix(), IndexConvert::get_desc_timestamp(*ts)),
            IndexType::ByTrigram(s, ts) => format!("{}:{:}/{:}/{:}", PREFIX_IDX, self.get_prefix(), s, IndexConvert::get_desc_timestamp(*ts)),
        }
    }
}

impl QueryRanges for Filter {
    fn get_index_bounds(&self) -> (Bound<String>, Bound<String>) {
        // use the index build over the text
        if let Some(text) = &self.text {
            if let Some(b) = Trigram::search_bound(&text) {
                let start = IndexType::ByTrigram(b.clone(), 0).get_index_key();
                let now = IndexType::ByTrigram(b, Utc::now().naive_utc().timestamp_millis() as u64).get_index_key();
                // timestamp index is built on descending order
                return (Bound::Included(now), Bound::Included(start))
            }
        }

        // just scan everythign for other queries
        let now = IndexType::Everything(Utc::now().naive_utc().timestamp_millis() as u64).get_index_key();
        let start = IndexType::Everything(0).get_index_key();
        // timestamp index is built on descending order
        (Bound::Included(now), Bound::Included(start))
    }
}

impl IndexedValue<IndexType> for proto_BookItem {

    fn get_index(&self) -> Vec<IndexType> {
        let mut text = String::new();

        let mut keys: Vec<IndexType> = Vec::new();
        let ts = self.create_timestamp;

        keys.push(IndexType::Everything(ts));

        let label = self.get_label().trim();
        if !label.is_empty() {
            text.push_str(label);
        }

        let address = &self.get_address().address.trim();
        if !address.is_empty() {
            text.push_str(address);
            keys.push(IndexType::ByAddress(address.to_lowercase().to_string(), ts));
        }

        let trigrams = Trigram::extract(text);
        trigrams.iter().for_each(|w| {
            keys.push(IndexType::ByTrigram(w.clone(), ts));
        });

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

impl AddressBookAccess {
    fn add_item(&self, item: proto_BookItem, batch: &mut Batch) -> Result<(), StateError> {
        let id = Uuid::parse_str(item.get_id()).unwrap();
        if let Ok(item_bytes) = item.write_to_bytes() {
            let item_key = AddressBookAccess::get_key(id);
            let indexes: Vec<String> = item.get_index_keys();
            Indexing::add_backrefs(&indexes, item_key.clone(), batch)?;
            for idx in indexes {
                batch.insert(idx.as_bytes(), item_key.as_bytes());
            }
            batch.insert(item_key.as_bytes(), item_bytes);
            Ok(())
        } else {
            Err(StateError::CorruptedValue)
        }
    }
}

impl AddressBook for AddressBookAccess {

    fn add(&self, items: Vec<proto_BookItem>) -> Result<Vec<Uuid>, StateError> {
        for item in &items {
            item.validate()?;
        }

        let mut batch = Batch::default();
        let mut ids = Vec::new();
        for item_source in items {
            // reuse existing id, or create a new id
            let mut item = if Uuid::parse_str(item_source.get_id()).is_ok() {
                item_source
            } else {
                let mut copy = item_source.clone();
                copy.set_id(Uuid::new_v4().to_string());
                copy
            };
            let id = Uuid::parse_str(item.get_id()).unwrap();

            // if it's just a newly created record then fill it with creation/update timestamp
            let now = Utc::now().naive_utc().timestamp_millis() as u64;
            if item.get_create_timestamp() == 0 {
                item.set_create_timestamp(now);
            }
            if item.get_update_timestamp() == 0 {
                item.set_update_timestamp(now);
            }

            let _ = self.add_item(item, &mut batch)?;
            ids.push(id);
        }
        self.db.apply_batch(batch)
            .map_err(|e| StateError::from(e))
            .map(|_| ids)
    }

    fn get(&self, id: Uuid) -> Result<Option<proto_BookItem>, StateError> {
        let item_key = AddressBookAccess::get_key(id);
        let result = self.db.get(item_key)?
            .map(|b| proto_BookItem::parse_from_bytes(b.as_ref()));
        match result {
            Some(parsed) => if let Ok(msg) = parsed {
                Ok(Some(msg))
            } else {
                Err(StateError::CorruptedValue)
            },
            None => Ok(None)
        }
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
        let mut bounds = filter.get_index_bounds();
        if let Some(cursor) = page.cursor {
            bounds.0 = Bound::Excluded(cursor.offset)
        };
        let mut processed = HashSet::new();
        let mut iter = self.db.range(bounds);
        let mut done = false;

        let mut results = Vec::new();
        let mut cursor_key: Option<String> = None;
        let mut read_count = 0;

        while !done {
            let next = iter.next();
            match next {
                Some(x) => match x {
                    Ok(v) => {
                        read_count += 1;

                        let idx_key = v.0.to_vec();
                        let idx_key = String::from_utf8(idx_key).unwrap();
                        cursor_key = Some(idx_key.clone());
                        let item_key = v.1.to_vec();
                        let item_key = AddressBookAccess::extract_id(String::from_utf8(item_key).unwrap())?;
                        let unprocessed = processed.insert(item_key.clone());
                        if unprocessed {
                            if let Some(item) = self.get_item(item_key) {
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

        let reached_end = read_count < page.limit;

        let result = PageResult {
            values: results,
            cursor: if reached_end { None } else { cursor_key.map(|offset| Cursor {offset}) },
        };

        Ok(result)
    }

    fn update(&self, id: Uuid, update: proto_BookItem) -> Result<(), StateError> {
        let mut batch = Batch::default();
        let item_key = AddressBookAccess::get_key(id);
        batch.remove(item_key.as_bytes());
        Indexing::remove_backref(item_key, self.db.clone(), &mut batch)?;

        let now = Utc::now().naive_utc().timestamp_millis() as u64;

        let mut item = update.clone();
        item.set_update_timestamp(now);
        item.set_id(id.to_string());
        let _ = self.add_item(item, &mut batch)?;

        self.db.apply_batch(batch)
            .map_err(|e| StateError::from(e))
    }
}

#[cfg(test)]
mod tests {
    use tempdir::TempDir;
    use uuid::Uuid;
    use chrono::Utc;
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
    fn create_and_get() {
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
        let id = results[0];

        let result = store.get(id);
        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.is_some());
        let mut result = result.unwrap();

        exp.id = id.clone().to_string();
        result.update_timestamp = 0;
        assert_eq!(result, exp);
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

    #[test]
    fn can_find_by_text() {
        let tmp_dir = TempDir::new("test-addressbook").unwrap();
        let access = SledStorage::open(tmp_dir.path().to_path_buf()).unwrap();
        let store = access.get_addressbook();

        let mut item = proto_BookItem::new();
        item.create_timestamp = 1_647_313_850_992;
        item.blockchain = 101;
        item.label = "Hello World!".to_string();
        let mut address = proto_Address::new();
        address.address = "0xEdD91797204D3537fBaBDe0E0E42AaE99975f2Bb".to_string();
        item.set_address(address);

        let results = store.add(vec![item.clone()]).expect("not saved");
        assert_eq!(results.len(), 1);
        let id = results[0].to_string();

        let filter = Filter {
            text: Some("world".to_string()),
            ..Filter::default()
        };

        let results = store.query(filter, PageQuery::default()).expect("queried");
        assert_eq!(results.values.len(), 1);
        let result = results.values.get(0).unwrap().clone();

        assert_eq!(result.id, id);
    }

    #[test]
    fn can_find_by_russian_text() {
        let tmp_dir = TempDir::new("test-addressbook").unwrap();
        let access = SledStorage::open(tmp_dir.path().to_path_buf()).unwrap();
        let store = access.get_addressbook();

        let mut item = proto_BookItem::new();
        item.create_timestamp = 1_647_313_850_992;
        item.blockchain = 101;
        item.label = "Привет Мир!".to_string();
        let mut address = proto_Address::new();
        address.address = "0xEdD91797204D3537fBaBDe0E0E42AaE99975f2Bb".to_string();
        item.set_address(address);

        let results = store.add(vec![item.clone()]).expect("not saved");
        assert_eq!(results.len(), 1);
        let id = results[0].to_string();

        let filter = Filter {
            text: Some("мир".to_string()),
            ..Filter::default()
        };

        let results = store.query(filter, PageQuery::default()).expect("queried");
        assert_eq!(results.values.len(), 1);
        let result = results.values.get(0).unwrap().clone();

        assert_eq!(result.id, id);
    }

    #[test]
    fn can_find_by_one_char_of_text() {
        let tmp_dir = TempDir::new("test-addressbook").unwrap();
        let access = SledStorage::open(tmp_dir.path().to_path_buf()).unwrap();
        let store = access.get_addressbook();

        let mut item = proto_BookItem::new();
        item.create_timestamp = 1_647_313_850_992;
        item.blockchain = 101;
        item.label = "Hello World!".to_string();
        let mut address = proto_Address::new();
        address.address = "0xEdD91797204D3537fBaBDe0E0E42AaE99975f2Bb".to_string();
        item.set_address(address);

        let results = store.add(vec![item.clone()]).expect("not saved");
        assert_eq!(results.len(), 1);
        let id = results[0].to_string();

        let filter = Filter {
            text: Some("h".to_string()),
            ..Filter::default()
        };

        let results = store.query(filter, PageQuery::default()).expect("queried");
        assert_eq!(results.values.len(), 1);
        let result = results.values.get(0).unwrap().clone();

        assert_eq!(result.id, id);
    }

    #[test]
    fn can_find_by_address_part() {
        let tmp_dir = TempDir::new("test-addressbook").unwrap();
        let access = SledStorage::open(tmp_dir.path().to_path_buf()).unwrap();
        let store = access.get_addressbook();

        let mut item = proto_BookItem::new();
        item.create_timestamp = 1_647_313_850_992;
        item.blockchain = 101;
        item.label = "Hello World!".to_string();
        let mut address = proto_Address::new();
        address.address = "0xEdD91797204D3537fBaBDe0E0E42AaE99975f2Bb".to_string();
        item.set_address(address);

        let results = store.add(vec![item.clone()]).expect("not saved");
        assert_eq!(results.len(), 1);
        let id = results[0].to_string();

        let filter = Filter {
            text: Some("9179".to_string()),
            ..Filter::default()
        };

        let results = store.query(filter, PageQuery::default()).expect("queried");
        assert_eq!(results.values.len(), 1);
        let result = results.values.get(0).unwrap().clone();

        assert_eq!(result.id, id);
    }

    #[test]
    fn updates_existing_entry() {
        let tmp_dir = TempDir::new("test-addressbook").unwrap();
        let access = SledStorage::open(tmp_dir.path().to_path_buf()).unwrap();
        let store = access.get_addressbook();

        let ts_start = Utc::now().naive_utc().timestamp_millis() as u64;

        let mut item = proto_BookItem::new();
        item.blockchain = 101;
        let mut address = proto_Address::new();
        address.address = "0xEdD91797204D3537fBaBDe0E0E42AaE99975f2Bb".to_string();
        item.set_address(address);
        let results = store.add(vec![item.clone()]).expect("not saved");
        let id = results[0];

        let mut updated = item.clone();
        updated.id = id.to_string();
        updated.label = "Hello World!".to_string();
        store.update(id, updated.clone()).expect("not updated");

        let ts_end = Utc::now().naive_utc().timestamp_millis() as u64;

        let exp = updated.clone();

        let results = store.query(Filter::default(), PageQuery::default()).expect("queried");
        assert_eq!(results.values.len(), 1);
        let mut result = results.values.get(0).unwrap().clone();

        assert!(result.update_timestamp >= ts_start);
        assert!(result.update_timestamp <= ts_end);

        result.clear_update_timestamp();
        assert_eq!(result, exp);
    }

    #[test]
    fn search_by_updated_label() {
        let tmp_dir = TempDir::new("test-addressbook").unwrap();
        let access = SledStorage::open(tmp_dir.path().to_path_buf()).unwrap();
        let store = access.get_addressbook();

        let mut item = proto_BookItem::new();
        item.blockchain = 101;
        let mut address = proto_Address::new();
        address.address = "0xEdD91797204D3537fBaBDe0E0E42AaE99975f2Bb".to_string();
        item.set_address(address);
        let results = store.add(vec![item.clone()]).expect("not saved");
        let id = results[0];

        let mut updated = item.clone();
        updated.id = id.to_string();
        updated.label = "Hello World!".to_string();
        store.update(id, updated.clone()).expect("not updated");

        let filter = Filter {
            text: Some("Hello".to_string()),
            ..Filter::default()
        };
        let results = store.query(filter, PageQuery::default()).expect("queried");
        assert_eq!(results.values.len(), 1);
        assert_eq!(results.values[0].id, id.to_string())

    }

    #[test]
    fn uses_cursor() {
        let tmp_dir = TempDir::new("test-addressbook").unwrap();
        let access = SledStorage::open(tmp_dir.path().to_path_buf()).unwrap();
        let store = access.get_addressbook();

        for i in 0..10 {
            let mut item = proto_BookItem::new();
            item.create_timestamp = 1_647_313_850_000 - i;
            item.blockchain = 101;
            item.label = format!("Hello World! {}", i);
            let mut address = proto_Address::new();
            address.address = format!("0xEdD91797204D3537fBaBDe0E0E42AaE99975f00{}", i);
            item.set_address(address);

            let _ = store.add(vec![item.clone()]).expect("not saved");
        }


        let results_1 = store.query(
            Filter {
                text: Some("world".to_string()),
                ..Filter::default()
            },
            PageQuery { limit: 5, ..PageQuery::default() }
        ).expect("queried");


        assert_eq!(results_1.values.len(), 5);
        assert_eq!(results_1.values[0].label, "Hello World! 0");
        assert_eq!(results_1.values[4].label, "Hello World! 4");
        assert!(results_1.cursor.is_some());

        let results_2 = store.query(
            Filter {
                text: Some("world".to_string()),
                ..Filter::default()
            },
            PageQuery { limit: 5, cursor: results_1.cursor, ..PageQuery::default() }
        ).expect("queried");


        assert_eq!(results_2.values.len(), 5);
        assert_eq!(results_2.values[0].label, "Hello World! 5");
        assert_eq!(results_2.values[4].label, "Hello World! 9");
        assert!(results_2.cursor.is_some()); // because it doesn't know yet that there is no other entries

        let results_3 = store.query(
            Filter {
                text: Some("world".to_string()),
                ..Filter::default()
            },
            PageQuery { limit: 5, cursor: results_2.cursor, ..PageQuery::default() }
        ).expect("queried");
        assert!(results_3.cursor.is_none());

    }


    #[test]
    fn validates_address() {
        let tmp_dir = TempDir::new("test-addressbook").unwrap();
        let access = SledStorage::open(tmp_dir.path().to_path_buf()).unwrap();
        let store = access.get_addressbook();

        let mut item = proto_BookItem::new();
        item.create_timestamp = 1_647_313_850_992;
        item.blockchain = 101;
        let mut address = proto_Address::new();
        address.address = "INVALID!!!".to_string();
        item.set_address(address);

        let results = store.add(vec![item.clone()]);
        assert!(results.is_err());

        let results = store.query(Filter::default(), PageQuery::default()).expect("queried");
        assert!(results.values.is_empty());
    }
}