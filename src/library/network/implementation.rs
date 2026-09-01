use crate::domain::config::port::NetworkInfo;

use embassy_executor::{Spawner, task};
use embassy_net::{Config, Runner, Stack, StackResources};
use embassy_time::{Duration, Timer};
use esp_hal::peripherals::{ADC1, LPWR, RNG, WIFI};
use esp_hal::rng::Rng;
use esp_hal::rtc_cntl::Rtc;
use esp_radio::wifi::sta::StationConfig;
use esp_radio::wifi::{Config as WifiConfig, ControllerConfig, Interface, WifiController};
use mbedtls_rs::Tls;
use mbedtls_rs::sys::hook::backend::embassy::timer::EmbassyTimer;
use mbedtls_rs::sys::hook::backend::esp::wall_clock::EspRtcWallClock;
use mbedtls_rs::sys::hook::{timer::hook_timer, wall_clock::hook_wall_clock};
use static_cell::StaticCell;

pub const ROOT_CA: &[u8] = concat!(include_str!("../../../res/isrg-root-x1.pem"), "\0").as_bytes();

static NETWORK_STACK_RESOURCES: StaticCell<StackResources<3>> = StaticCell::new();
static RTC: StaticCell<Rtc<'static>> = StaticCell::new();
static TRNG_SOURCE: StaticCell<esp_hal::rng::TrngSource<'static>> = StaticCell::new();
static TRNG: StaticCell<esp_hal::rng::Trng> = StaticCell::new();
static TLS_TIMER: StaticCell<EmbassyTimer> = StaticCell::new();
static TLS_CLOCK: StaticCell<EspRtcWallClock<&'static Rtc<'static>>> = StaticCell::new();

pub struct NetworkStack {
    pub stack: Stack<'static>,
    pub rtc: &'static Rtc<'static>,
    pub tls: Tls<'static>,
}

impl NetworkStack {
    pub fn new(wifi: WIFI<'static>, lpwr: LPWR<'static>, rng: RNG<'static>, adc1: ADC1<'static>, network_info: &NetworkInfo, spawner: Spawner) -> Self {
        let station = WifiConfig::Station(StationConfig::default().with_ssid(network_info.wifi_ssid.as_str()).with_password(network_info.wifi_password.as_str().into()));
        let controller_config = ControllerConfig::default().with_initial_config(station);
        let (controller, interfaces) = esp_radio::wifi::new(wifi, controller_config).unwrap();
        let random = Rng::new();
        let (stack, runner) = embassy_net::new(interfaces.station, Config::dhcpv4(Default::default()), NETWORK_STACK_RESOURCES.init(StackResources::new()), (random.random() as u64) << 32 | random.random() as u64);
        let rtc = RTC.init(Rtc::new(lpwr));

        TRNG_SOURCE.init(esp_hal::rng::TrngSource::new(rng, adc1));

        let trng = TRNG.init(esp_hal::rng::Trng::try_new().unwrap());

        unsafe {
            hook_timer(Some(TLS_TIMER.init(EmbassyTimer)));
            hook_wall_clock(Some(TLS_CLOCK.init(EspRtcWallClock::new(rtc))));
        }

        spawner.spawn(connection_task(controller).unwrap());
        spawner.spawn(network_task(runner).unwrap());

        Self { stack, rtc, tls: Tls::new(trng).unwrap() }
    }
}

#[task]
async fn connection_task(mut controller: WifiController<'static>) {
    loop {
        if controller.connect_async().await.is_ok() {
            let _ = controller.wait_for_disconnect_async().await;
        }

        Timer::after(Duration::from_secs(5)).await;
    }
}

#[task]
async fn network_task(mut runner: Runner<'static, Interface<'static>>) {
    runner.run().await;
}
