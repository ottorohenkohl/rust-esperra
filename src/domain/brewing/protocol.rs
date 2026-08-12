#[derive(Clone, Copy, Debug, PartialEq)]
pub enum BrewMessage {
    SetBrewing(bool),
    SetPaused(bool),
}
