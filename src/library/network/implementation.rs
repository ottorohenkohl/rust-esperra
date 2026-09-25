use super::protocol::NetworkError;

use super::task::{connection_task, network_task};

use crate::domain::config::port::NetworkInfo;

use embassy_executor::Spawner;
use core::net::{IpAddr, SocketAddr};
use embassy_net::dns::DnsQueryType;
use embassy_net::udp::{PacketMetadata, UdpSocket};
use embassy_net::{Config, Stack, StackResources};
use esp_hal::rng::Rng;
use esp_hal::rtc_cntl::Rtc;
use esp_radio::wifi::sta::StationConfig;
use esp_radio::wifi::ControllerConfig;
use mbedtls_rs::Tls;
use mbedtls_rs::sys::hook::backend::embassy::timer::EmbassyTimer;
use mbedtls_rs::sys::hook::backend::esp::wall_clock::EspRtcWallClock;
use sntpc::NtpContext;
use sntpc_net_embassy::UdpSocketWrapper;
use sntpc_time_embassy::EmbassyTimestampGenerator;
use static_cell::StaticCell;

use esp_radio::wifi::Config as WifiConfig;

use mbedtls_rs::sys::hook::{timer::hook_timer, wall_clock::hook_wall_clock};
use sntpc::get_time;

use esp_hal::peripherals::{ADC1, LPWR, RNG, WIFI};

pub const ROOT_CA: &[u8] = concat!(include_str!("../../../res/isrg-root-x1.pem"), "\0").as_bytes();

static NETWORK_STACK_RESOURCES: StaticCell<StackResources<3>> = StaticCell::new();
static RTC: StaticCell<Rtc<'static>> = StaticCell::new();
static TRNG_SOURCE: StaticCell<esp_hal::rng::TrngSource<'static>> = StaticCell::new();
static TRNG: StaticCell<esp_hal::rng::Trng> = StaticCell::new();
static TLS_TIMER: StaticCell<EmbassyTimer> = StaticCell::new();
static TLS_CLOCK: StaticCell<EspRtcWallClock<&'static Rtc<'static>>> = StaticCell::new();

pub struct NetworkStackImpl {
    pub stack: Stack<'static>,
    pub rtc: &'static Rtc<'static>,
    pub tls: Tls<'static>,
}

impl NetworkStackImpl {
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

    pub async fn synchronize_time(&self, ntp_server: &str) -> Result<(), NetworkError> {
        let addresses = self.stack.dns_query(ntp_server, DnsQueryType::A).await.map_err(|_| NetworkError::Dns)?;

        let mut rx_meta = [PacketMetadata::EMPTY; 1];
        let mut rx_buffer = [0; 128];
        let mut tx_meta = [PacketMetadata::EMPTY; 1];
        let mut tx_buffer = [0; 128];

        let mut socket = UdpSocket::new(self.stack, &mut rx_meta, &mut rx_buffer, &mut tx_meta, &mut tx_buffer);

        socket.bind(0).map_err(|_| NetworkError::Socket)?;

        let socket = UdpSocketWrapper::new(socket);
        let server = SocketAddr::from((IpAddr::from(*addresses.first().ok_or(NetworkError::Dns)?), 123));
        let context = NtpContext::new(EmbassyTimestampGenerator::default());

        let result = get_time(server, &socket, context).await.map_err(|_| NetworkError::Time)?;

        self.rtc.set_current_time_us(result.sec() as u64 * 1_000_000 + ((result.sec_fraction() as u64 * 1_000_000) >> 32));

        Ok(())
    }
}
