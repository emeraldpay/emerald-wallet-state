use std::str::FromStr;
use chrono::{DateTime, TimeZone, Utc};
use num_bigint::BigUint;
use num_traits::identities::Zero;
use crate::errors::{StateError};
use crate::proto::balance::{Balance as proto_Balance, BalanceBundle as proto_BalanceBundle, Utxo as proto_Utxo};

#[derive(Debug, Clone, PartialEq)]
pub struct Balance {
    pub amount: BigUint,
    pub ts: DateTime<Utc>,
    pub address: String,
    pub blockchain: u32,
    pub asset: String,
    pub utxo: Vec<Utxo>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Utxo {
    pub txid: String,
    pub vout: u32,
    pub amount: u64,
}

impl Default for Balance {
    fn default() -> Self {
        Balance {
            amount: BigUint::zero(),
            ts: Utc::now(),
            address: "NONE".to_string(),
            blockchain: 0,
            asset: "NONE".to_string(),
            utxo: vec![]
        }
    }
}

impl Balance {

    ///
    /// Make sure that the balance object is consistent.
    /// If it contains Utxo the sum of Utxo must equal the total amount.
    /// If it finds a utxo inconsitency it returns the balance w/o utxo
    fn validated(self) -> Balance {
        if self.utxo.is_empty() {
            self
        } else {
            let total: u64 = self.utxo.iter().map(|u| u.amount).sum();
            if BigUint::from(total) == self.amount {
                self
            } else {
                Balance {
                    utxo: vec![],
                    ..self
                }
            }
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
            ts: Utc.timestamp_millis_opt(value.ts as i64).unwrap(),
            address: value.address.clone(),
            blockchain: value.blockchain,
            asset: value.asset.clone(),
            utxo: value.utxo.to_vec().iter().map(|p| p.into()).collect()
        }.validated())
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
        proto.set_utxo(self.utxo.iter().map(|u| u.clone().into()).collect());

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

impl From<&proto_Utxo> for Utxo {

    fn from(value: &proto_Utxo) -> Self {
        Utxo {
            amount: value.get_amount(),
            txid: value.get_txid().to_string(),
            vout: value.get_vout()
        }
    }
}

impl Into<proto_Utxo> for Utxo {
    fn into(self) -> proto_Utxo {
        let mut proto = proto_Utxo::new();
        proto.set_txid(self.txid);
        proto.set_amount(self.amount);
        proto.set_vout(self.vout);
        proto
    }
}
