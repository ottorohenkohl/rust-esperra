pub trait InputPort {
    fn powered_state(&mut self) -> bool;
}
