use super::protocol::LightMode;

pub struct LightService {
    mode: LightMode,
    remaining_blinks: u8,
}

impl LightService {
    pub fn new() -> Self {
        Self { mode: LightMode::Off, remaining_blinks: 0 }
    }

    pub fn set_mode(&mut self, mode: LightMode) {
        self.mode = mode;
    }

    pub fn blink_times(&mut self, times: u8) {
        self.remaining_blinks = times;
    }

    pub fn cancel_blinks(&mut self) {
        self.remaining_blinks = 0;
    }

    pub fn active(&self, phase: bool) -> bool {
        if self.remaining_blinks > 0 {
            return phase
        }

        match self.mode {
            LightMode::Off => false,
            LightMode::On => true,
            LightMode::Blinking => phase,
        }
    }

    pub fn finish_pulse(&mut self) {
        self.remaining_blinks = self.remaining_blinks.saturating_sub(1);
    }
}
