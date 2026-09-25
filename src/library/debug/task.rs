use super::protocol::Color;

use embassy_executor::task;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use esp_hal::Async;
use esp_hal::gpio::Level;
use esp_hal::rmt::{Channel, PulseCode, Tx};

use embassy_sync::channel::Channel as EmbassyChannel;

#[task]
pub async fn debug_led_task(colors: &'static EmbassyChannel<CriticalSectionRawMutex, Color, 8>, color_applied: &'static EmbassyChannel<CriticalSectionRawMutex, (), 1>, rgb: fn(Color) -> (u8, u8, u8), mut channel48: Channel<'static, Async, Tx>, mut channel38: Channel<'static, Async, Tx>) {
    loop {
        let (red, green, blue) = rgb(colors.receive().await);

        let mut pulses = [PulseCode::default(); 25];
        let mut index = 0;

        for byte in [green, red, blue] {
            for bit in (0..8).rev() {
                pulses[index] = if byte & (1 << bit) == 0 {
                    PulseCode::new(Level::High, 28, Level::Low, 64)
                } else {
                    PulseCode::new(Level::High, 56, Level::Low, 48)
                };

                index += 1;
            }
        }

        pulses[24] = PulseCode::end_marker();

        let _ = channel48.transmit(&pulses).await;
        let _ = channel38.transmit(&pulses).await;

        color_applied.send(()).await;
    }
}
