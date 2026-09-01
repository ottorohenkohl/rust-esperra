use crate::domain::clock::port::TimerPort;

use embassy_time::{Timer, Duration};

pub struct EspTimerAdapter {}

impl EspTimerAdapter {
    pub fn new() -> Self {
        Self {}
    }
}

impl TimerPort for EspTimerAdapter {
    async fn wait_for(&self, millis: u64) {
        Timer::after(Duration::from_millis(millis)).await;
    }
}
