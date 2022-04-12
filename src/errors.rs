#[derive(Clone, Debug)]
pub enum StateError {
    IOError,
    InvalidId
}

impl From<sled::Error> for StateError {
    fn from(_: sled::Error) -> Self {
        StateError::IOError
    }
}

impl From<uuid::Error> for StateError {
    fn from(_: uuid::Error) -> Self {
        StateError::InvalidId
    }
}