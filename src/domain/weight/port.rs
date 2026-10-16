#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConnectionState {
    AlreadyConnected,
    ConnectionEstablished,
    ConnectionFailed,
}

pub trait WeightPort {
    async fn connect(&mut self) -> ConnectionState;
    async fn shutdown(&mut self);
    async fn reset_ready_state(&mut self);
    async fn is_ready_state(&mut self) -> bool;
    async fn read_weight(&mut self) -> f64;
}
