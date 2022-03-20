#[derive(Clone, Debug)]
pub enum StateError {
    IOError
}

impl From<sled::Error> for StateError {
    fn from(_: sled::Error) -> Self {
        StateError::IOError
    }
}