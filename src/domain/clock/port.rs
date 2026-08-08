use alloc::boxed::Box;

pub trait Instant {
    fn elapsed_ms(&self) -> u64;
    fn now(&self) -> Box<dyn Instant>;
}
