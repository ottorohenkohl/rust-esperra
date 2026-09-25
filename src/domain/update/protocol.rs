use heapless::String;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub enum UpdateMessage {
    Start,
    Publish,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub enum UpdateResponse {
    Version(String<32>),
}
