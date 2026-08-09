#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputState {
    Inactive,
    Active,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OutputCommand {
    Set(OutputState),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StatusLightCommand {
    Set(OutputState),
    BlinkOnce,
}
