#[derive(Clone, Copy, Debug, PartialEq, serde::Deserialize, serde::Serialize)]
pub enum BrewMessage {
    SetBrewing(bool),
}
