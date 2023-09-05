use uuid::Uuid;
use crate::access::pagination::PageResult;
use crate::errors::StateError;
use crate::proto::balance::Allowance;

///
/// Cache for ERC-20 allowance data
pub trait Allowances {

    ///
    /// Add an allowance to the cache
    ///
    /// - `allowance` - Allowance to add
    /// - `ttl` - Time to live in milliseconds (default 24 hours)
    fn add(&self, allowance: Allowance, ttl: Option<u64>) -> Result<(), StateError>;

    ///
    /// List allowances. If `wallet_id` is specified, only allowances for that wallet are returned.
    fn list(&self, wallet_id: Option<Uuid>) -> Result<PageResult<Allowance>, StateError>;

    ///
    /// Remove an allowance from the cache for the specified wallet and blockchain
    ///
    /// - `wallet_id` - Wallet ID
    /// - `blockchain` - Blockchain ID, if set only allowances for that blockchain are removed, otherwise any blockchain is removed
    /// - `min_ts` - Minimum timestamp (ms), if set only allowances with a timestamp lesser than this value are removed, otherwise any timestamp is removed
    fn remove(&self, wallet_id: Uuid, blockchain: Option<u32>, min_ts: Option<u64>) -> Result<usize, StateError>;
}