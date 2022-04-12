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