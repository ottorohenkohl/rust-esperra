pub trait TimerPort {
    async fn wait_for(&self, millis: u64);
}
