#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MessagingError {
    Config,
    Dns,
    Tcp,
    Tls,
    Mqtt,
}
