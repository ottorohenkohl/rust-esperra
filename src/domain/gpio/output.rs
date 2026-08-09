#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputMessage {
    SetActive(bool),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LightMessage {
    SetActive(bool),
    BlinkOnce,
}
