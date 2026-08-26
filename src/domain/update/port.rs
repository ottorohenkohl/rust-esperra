#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UpdateConfirmation {
    Confirmed,
    NotPending,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UpdateError {
    Flash,
    Http,
    InvalidImage,
    InvalidPartitionTable,
    Time,
}

pub trait UpdatePort {
    async fn update(&mut self) -> Result<(), UpdateError>;

    fn confirm(&mut self) -> Result<UpdateConfirmation, UpdateError>;
}
