use crate::domain::clock::Instant;

pub struct EmbassyInstant {
    started_at: embassy_time::Instant,
}

impl EmbassyInstant {
    pub fn now() -> Self {
        Self { started_at: embassy_time::Instant::now() }
    }
}

impl Instant for EmbassyInstant {
    fn elapsed_ms(&self) -> u64 {
        self.started_at.elapsed().as_millis()
    }

    fn restart(&mut self) {
        self.started_at = embassy_time::Instant::now();
    }
}
