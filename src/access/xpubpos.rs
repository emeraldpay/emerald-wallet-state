use crate::errors::StateError;

///
/// Keep state of a XPub index
pub trait XPubPosition {

    ///
    /// Remember that the specified `xpub` is at least at position `pos`. I.e., that address is used.
    /// If the currently stored state has a larger value it stays as is, if a lower value - set with the provided
    fn set_at_least(&self, xpub: String, pos: u32) -> Result<(), StateError>;

    ///
    /// Get current know position for the `xpub`
    fn get(&self, xpub: String) -> Result<Option<u32>, StateError>;

    ///
    /// Get current know position for the `xpub`. Returns zero if no position is known, assuming it just starts.
    fn get_next(&self, xpub: String) -> Result<u32, StateError>;

}