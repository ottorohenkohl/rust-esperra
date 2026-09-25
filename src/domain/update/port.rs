use heapless::String;

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
    fn current_version(&self) -> String<32>;

    async fn update(&mut self) -> Result<UpdateConfirmation, UpdateError>;

    async fn confirm(&mut self) -> Result<UpdateConfirmation, UpdateError>;
}
