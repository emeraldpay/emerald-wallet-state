use std::sync::Arc;
use sled::{Db, IVec};
use crate::access::xpubpos::XPubPosition;
use crate::errors::{InvalidValueError, StateError};

const PREFIX_KEY: &'static str = "xpubpos:";

pub struct XPubPositionAccess {
    pub(crate) db: Arc<Db>,
}

impl XPubPositionAccess {

    /// Checks if the `xpub` can be used as a key
    fn is_valid<S: AsRef<str>>(xpub: S) -> bool {
        // we don't really need to do a full validation, just check that it can be a valid key
        return xpub.as_ref().chars().all(|c| c.is_ascii_alphanumeric())
    }

    /// Storage key for the `xpub`. Also validates the `xpub` value.
    fn key(xpub: String) -> Result<String, StateError> {
        if XPubPositionAccess::is_valid(&xpub) {
            Ok(format!("{}{}", PREFIX_KEY, xpub))
        } else {
            Err(StateError::InvalidValue(InvalidValueError::Name("xpub".to_string())))
        }
    }

    /// Convert from stored value to number.
    /// NOTE: if stored is empty or invalid it returns 0
    fn deserialize(value: &IVec) -> u32 {
        let slice = value.as_ref();
        // if we see we cannot decode it - return 0
        if slice.len() > 4 || slice.len() == 0 {
            return 0u32
        }
        // convert to a correct size slice, so u32 can read from it
        let mut slice_4 = [0u8; 4];
        let pos = 4 - slice.len();
        slice_4[pos..].copy_from_slice(slice);
        u32::from_be_bytes(slice_4)
    }

    /// Convert from number to stored value
    fn serialize(value: u32) -> IVec {
        let slice = u32::to_be_bytes(value);
        IVec::from(&slice)
    }
}

impl XPubPosition for XPubPositionAccess {
    fn set_at_least(&self, xpub: String, pos: u32) -> Result<(), StateError> {
        let key = XPubPositionAccess::key(xpub)?;
        let mut updated = false;
        while !updated {
            let prev = self.db.get(&key)?;
            let next = match prev.as_ref().map(|b| XPubPositionAccess::deserialize(b) ) {
                None => pos,
                Some(existing) => if existing == pos {
                    // already contains the same value, doesn't need to be updated
                    return Ok(())
                } else {
                    // otherwise try to find the largest of current and proposed
                    if existing < pos { pos } else { existing }
                }
            };
            let result = self.db.compare_and_swap(&key, prev, Some(XPubPositionAccess::serialize(next)))?;
            updated = result.is_ok();
        }
        Ok(())
    }

    fn get(&self, xpub: String) -> Result<Option<u32>, StateError> {
        let key = XPubPositionAccess::key(xpub)?;
        let current = self.db.get(&key)?
            .map(|b| XPubPositionAccess::deserialize(&b) );
        Ok(current)
    }

    fn get_next(&self, xpub: String) -> Result<u32, StateError> {
        let current = self.get(xpub)?;
        match current {
            Some(v) => Ok(v + 1),
            None => Ok(0u32)
        }
    }
}

#[cfg(test)]
mod tests {
    use tempdir::TempDir;
    use crate::access::xpubpos::XPubPosition;
    use crate::storage::sled_access::SledStorage;
    use crate::storage::xpubpos_store::XPubPositionAccess;

    #[test]
    fn actual_xpub_is_valid() {
        let xpub = "xpub6Ea1EGxsjJbbNvWvX6DsFKg2DzX1mryk8GaRB86BxC6VAtwUpKtL8nyQbMkonyiB28KUVLk5qYncZfFvmXTKdktntdgPdzoyBSFvMvCzdY1";
        assert!(XPubPositionAccess::is_valid(xpub))
    }

    #[test]
    fn actual_xpub_bip84_is_valid() {
        let xpub = "zpub6tWCR2jxaKabCC5rHL8skXr6HsqLY58oihn7Dm6pTvNSa4gpde5T2eQT12Wid8h3ygM5yWWwSzbjmFRGHut6JBPDD6kaESPsQCrGSMSSwJy";
        assert!(XPubPositionAccess::is_valid(xpub))
    }

    #[test]
    fn garbage_xpub_is_not_valid() {
        let xpub = "hello world";
        assert!(!XPubPositionAccess::is_valid(xpub))
    }

    #[test]
    fn serialize_and_deserialize_back() {
        let numbers = vec![0u32, 1, 2, 5, 17, 100, 127, 128, 200, 255, 256, 300, 1000, 65535, 65536, 70000];
        for n in numbers {
            let s = XPubPositionAccess::serialize(n);
            let d = XPubPositionAccess::deserialize(&s);
            assert_eq!(n, d);
        }
    }

    #[test]
    fn serialize_be() {
        assert_eq!(
            hex::encode(XPubPositionAccess::serialize(0)),
            "00000000"
        );

        assert_eq!(
            hex::encode(XPubPositionAccess::serialize(1000)),
            "000003e8"
        );
    }

    #[test]
    fn updates_value() {
        let tmp_dir = TempDir::new("xpubpos").unwrap();
        let store = SledStorage::open(tmp_dir.path().to_path_buf()).unwrap();
        let access = store.get_xpub_pos();
        let xpub = "zpub6tWCR2jxaKabCC5rHL8skXr6HsqLY58oihn7Dm6pTvNSa4gpde5T2eQT12Wid8h3ygM5yWWwSzbjmFRGHut6JBPDD6kaESPsQCrGSMSSwJy".to_string();
        access.set_at_least(xpub.clone(), 1).unwrap();

        let value = access.get(xpub.clone()).unwrap();
        assert_eq!(value, Some(1));

        access.set_at_least(xpub.clone(), 3).unwrap();
        let value = access.get(xpub.clone()).unwrap();
        assert_eq!(value, Some(3));
    }

    #[test]
    fn skip_low_value() {
        let tmp_dir = TempDir::new("xpubpos").unwrap();
        let store = SledStorage::open(tmp_dir.path().to_path_buf()).unwrap();
        let access = store.get_xpub_pos();
        let xpub = "zpub6tWCR2jxaKabCC5rHL8skXr6HsqLY58oihn7Dm6pTvNSa4gpde5T2eQT12Wid8h3ygM5yWWwSzbjmFRGHut6JBPDD6kaESPsQCrGSMSSwJy".to_string();

        access.set_at_least(xpub.clone(), 5).unwrap();
        let value = access.get(xpub.clone()).unwrap();
        assert_eq!(value, Some(5));

        access.set_at_least(xpub.clone(), 3).unwrap();
        let value = access.get(xpub.clone()).unwrap();
        assert_eq!(value, Some(5));
    }

    #[test]
    fn current_is_nothing_by_default() {
        let tmp_dir = TempDir::new("xpubpos").unwrap();
        let store = SledStorage::open(tmp_dir.path().to_path_buf()).unwrap();
        let access = store.get_xpub_pos();
        let xpub = "zpub6tWCR2jxaKabCC5rHL8skXr6HsqLY58oihn7Dm6pTvNSa4gpde5T2eQT12Wid8h3ygM5yWWwSzbjmFRGHut6JBPDD6kaESPsQCrGSMSSwJy".to_string();

        let value = access.get(xpub.clone()).unwrap();
        assert_eq!(value, None);
    }

    #[test]
    fn next_is_zero_by_default() {
        let tmp_dir = TempDir::new("xpubpos").unwrap();
        let store = SledStorage::open(tmp_dir.path().to_path_buf()).unwrap();
        let access = store.get_xpub_pos();
        let xpub = "zpub6tWCR2jxaKabCC5rHL8skXr6HsqLY58oihn7Dm6pTvNSa4gpde5T2eQT12Wid8h3ygM5yWWwSzbjmFRGHut6JBPDD6kaESPsQCrGSMSSwJy".to_string();

        let value = access.get_next(xpub.clone()).unwrap();
        assert_eq!(value, 0);
    }

    #[test]
    fn next_is_after_current() {
        let tmp_dir = TempDir::new("xpubpos").unwrap();
        let store = SledStorage::open(tmp_dir.path().to_path_buf()).unwrap();
        let access = store.get_xpub_pos();
        let xpub = "zpub6tWCR2jxaKabCC5rHL8skXr6HsqLY58oihn7Dm6pTvNSa4gpde5T2eQT12Wid8h3ygM5yWWwSzbjmFRGHut6JBPDD6kaESPsQCrGSMSSwJy".to_string();

        access.set_at_least(xpub.clone(), 5).unwrap();
        let value = access.get_next(xpub.clone()).unwrap();
        assert_eq!(value, 6);
    }
}