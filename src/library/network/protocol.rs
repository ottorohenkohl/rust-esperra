#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NetworkError {
    Dns,
    Socket,
    Time,
}
