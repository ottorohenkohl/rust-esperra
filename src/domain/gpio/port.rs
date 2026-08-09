pub trait InputDevice {
    fn powered_state(&mut self) -> bool;
}
