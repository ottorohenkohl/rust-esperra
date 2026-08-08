pub trait InputDevice {
    fn powered_state(&mut self) -> bool;
}

pub trait OutputDevice {
    fn blink_once(&mut self);
    fn set_active(&mut self, active: bool);
}

pub trait TemperatureSensor {
    fn read_temperature(&mut self) -> f64;
}
