#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConnectionState {
    AlreadyConnected,
    ConnectionEstablished,
    ConnectionFailed,
}

pub trait WeightPort {
    async fn connect(&mut self) -> ConnectionState;
    async fn tare(&mut self);
    async fn read_weight(&mut self) -> f64;
}
