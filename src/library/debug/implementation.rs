use super::protocol::Color;

use super::task::debug_led_task;

use embassy_executor::Spawner;
use embassy_sync::blocking_mutex::raw::CriticalSectionRawMutex;
use embassy_time::{Duration, Timer};
use esp_hal::gpio::Level;
use esp_hal::rmt::{Rmt, TxChannelConfig, TxChannelCreator};
use esp_hal::time::Rate;

use embassy_sync::channel::Channel as EmbassyChannel;

use esp_hal::peripherals::{GPIO38, GPIO48, RMT};

static COLORS: EmbassyChannel<CriticalSectionRawMutex, Color, 8> = EmbassyChannel::new();
static COLOR_APPLIED: EmbassyChannel<CriticalSectionRawMutex, (), 1> = EmbassyChannel::new();

pub struct DebugImpl;

impl DebugImpl {
    pub fn initialize(rmt: RMT<'static>, pin48: GPIO48<'static>, pin38: GPIO38<'static>, spawner: Spawner) {
        let rmt = Rmt::new(rmt, Rate::from_mhz(80)).unwrap().into_async();
        let config = TxChannelConfig::default().with_clk_divider(1).with_idle_output(true).with_idle_output_level(Level::Low);

        let channel48 = rmt.channel0.configure_tx(&config).unwrap().with_pin(pin48);
        let channel38 = rmt.channel1.configure_tx(&config).unwrap().with_pin(pin38);

        spawner.spawn(debug_led_task(&COLORS, &COLOR_APPLIED, Self::rgb, channel48, channel38).unwrap());
    }

    pub async fn set(color: Color) {
        COLORS.send(color).await;
        COLOR_APPLIED.receive().await;

        Timer::after(Duration::from_millis(500)).await;
    }

    fn rgb(color: Color) -> (u8, u8, u8) {
        match color {
            Color::Red => (24, 0, 0),
            Color::Magenta => (24, 0, 12),
            Color::Yellow => (20, 16, 0),
            Color::White => (12, 12, 12),
            Color::Green => (0, 24, 0),
            Color::Cyan => (0, 16, 16),
            Color::Blue => (0, 0, 24),
        }
    }
}
