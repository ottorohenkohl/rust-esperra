pub trait InputDevice {
    fn powered_state(&self) -> bool;
}

pub trait OutputDevice {
    fn blink_once(&self);
    fn set_active(&self, active: bool);
}

pub trait TemperatureSensor {
    fn read_temperature(&self) -> f64;
}
