#[derive(Clone, Debug)]
pub enum StateError {
    IOError,
    InvalidId,
    InvalidValue(InvalidValueError),
}

#[derive(Clone, Debug)]
pub enum InvalidValueError {
    Name(String),
    NameMessage(String, String),
    Other(String),
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

impl From<InvalidValueError> for StateError {
    fn from(e: InvalidValueError) -> Self {
        StateError::InvalidValue(e)
    }
}