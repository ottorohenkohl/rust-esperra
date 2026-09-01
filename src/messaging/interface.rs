use serde::Serialize;

#[derive(Serialize)]
pub enum MessagingEvent {
    Connected,
    CommandAccepted,
    BadFormat,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MessagingError {
    Config,
    Dns,
    Tcp,
    Tls,
    Mqtt,
}
