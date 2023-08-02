use crate::errors::{InvalidValueError, StateError};
use regex::Regex;
use lazy_static::lazy_static;

lazy_static! {
    static ref ETHEREUM_ADDRESS_REGEX: Regex = Regex::new(r"^0x[a-fA-F0-9]{40}$").unwrap();
}

pub(crate) fn check_ethereum_address(address: &str) -> Result<(), StateError> {
    if !ETHEREUM_ADDRESS_REGEX.is_match(address) {
        return Err(StateError::InvalidValue(
            InvalidValueError::NameMessage("address".to_string(), "invalid".to_string())))
    }
    Ok(())
}

pub(crate) fn check_bitcoin_address(address: &str) -> Result<(), StateError> {
    if !address.is_ascii() {
        return Err(StateError::InvalidValue(
            InvalidValueError::NameMessage("address".to_string(), "non-ascii".to_string())))
    }
    Ok(())
}

pub(crate) fn check_address(address: &str) -> Result<(), StateError> {
    if check_ethereum_address(address).is_ok() {
        return Ok(())
    }
    if check_bitcoin_address(address).is_ok() {
        return Ok(())
    }
    return Err(StateError::InvalidValue(
        InvalidValueError::NameMessage("address".to_string(), "invalid".to_string())))
}

#[cfg(test)]
mod tests {
    use crate::validate::check_ethereum_address;
    use crate::validate::check_address;

    #[test]
    fn accept_valid_address() {
        assert_eq!(check_ethereum_address("0x65A0947BA5175359Bb457D3b34491eDf4cBF7997"), Ok(()));
        assert_eq!(check_ethereum_address("0xdac17f958d2ee523a2206206994597c13d831ec7"), Ok(()));
    }

    #[test]
    fn deny_invalid_address() {
        assert!(check_ethereum_address("65A0947BA5175359Bb457D3b34491eDf4cBF7997").is_err());
        assert!(check_ethereum_address("0x958d2ee523a2206206994597c13d831ec7").is_err());
    }

    #[test]
    fn accept_bitcoin_address() {
        assert_eq!(check_address("bc1q2dz68vuh65h4tmp7kla5lrq907kqx0fwfccwqd"), Ok(()));
        assert_eq!(check_address("bc1qpnvms2g7a72sz0xdpkrgjtw9llldr0nw8z0e765yzch65fp637vqtvt248"), Ok(()));
        assert_eq!(check_address("3JudqvZAr6X2z1BxhnPxajZNdwC9vfP8wb"), Ok(()));
    }

}