use std::str::FromStr;
use chrono::{DateTime, TimeZone, Utc};
use num_bigint::BigUint;
use num_traits::identities::Zero;
use crate::errors::StateError;
use crate::proto::balance::{Balance as proto_Balance, BalanceBundle as proto_BalanceBundle};

#[derive(Debug, Clone, PartialEq)]
pub struct Balance {
    pub amount: BigUint,
    pub ts: DateTime<Utc>,
    pub address: String,
    pub blockchain: u32,
    pub asset: String,
}

impl Default for Balance {
    fn default() -> Self {
        Balance {
            amount: BigUint::zero(),
            ts: Utc::now(),
            address: "NONE".to_string(),
            blockchain: 0,
            asset: "NONE".to_string()
        }
    }
}

pub(crate) fn concat(base: Vec<Balance>, extra: Balance) -> Vec<Balance> {
    let mut result = Vec::new();
    for b in base {
        if b.blockchain != extra.blockchain || b.asset != extra.asset {
            result.push(b)
        }
    }
    result.push(extra);
    result
}


///
/// Balances cache
pub trait Balances {

    ///
    /// Set current value. It merges multiple balances per address in one list, so all of them fetched in bulk later
    fn set(&self, value: Balance) -> Result<(), StateError>;

    ///
    /// List all known balances per address. The address is supposed to be a single address, not a XPub
    fn list(&self, address: String) -> Result<Vec<Balance>, StateError>;

    /// Clear all known balances per address
    fn clear(&self, address: String) -> Result<(), StateError>;

}

impl TryFrom<&proto_Balance> for Balance {
    type Error = StateError;

    fn try_from(value: &proto_Balance) -> Result<Self, Self::Error> {
        Ok(Balance {
            amount: BigUint::from_str(value.amount.as_str())
                .map_err(|_| StateError::CorruptedValue)?,
            ts: Utc.timestamp_millis(value.ts as i64),
            address: value.address.clone(),
            blockchain: value.blockchain,
            asset: value.asset.clone(),
        })
    }
}

impl Into<proto_Balance> for Balance {
    fn into(self) -> proto_Balance {
        let mut proto = proto_Balance::new();

        proto.set_amount(self.amount.to_string());
        proto.set_ts(self.ts.timestamp_millis() as u64);
        proto.set_address(self.address);
        proto.set_blockchain(self.blockchain);
        proto.set_asset(self.asset);

        proto
    }
}

impl From<proto_BalanceBundle> for Vec<Balance> {
    fn from(value: proto_BalanceBundle) -> Self {
        let mut result = Vec::new();
        value.balances.iter()
            .for_each(|b| {
                if let Ok(parsed) = Balance::try_from(b) {
                    result.push(parsed)
                }
            });
        result
    }
}

impl Into<proto_BalanceBundle> for Vec<Balance> {
    fn into(self) -> proto_BalanceBundle {
        let mut proto = proto_BalanceBundle::new();
        for b in self {
            proto.balances.push(b.into());
        }
        proto
    }
}

