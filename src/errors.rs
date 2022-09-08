use protobuf::ProtobufError;

#[derive(Clone, Debug, PartialEq)]
pub enum StateError {
    IOError,
    InvalidId,
    InvalidValue(InvalidValueError),
    CorruptedValue,
}

#[derive(Clone, Debug, PartialEq)]
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

impl From<ProtobufError> for StateError {
    fn from(_: ProtobufError) -> Self {
        StateError::CorruptedValue
    }
}