pub trait Instant {
    fn elapsed_ms(&self) -> u64;
    fn restart(&mut self);
}
