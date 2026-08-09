#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputState {
    Inactive,
    Active,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputMessage {
    Set(OutputState),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StatusLightMessage {
    Set(OutputState),
    BlinkOnce,
}
