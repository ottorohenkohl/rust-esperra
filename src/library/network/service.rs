use super::implementation::NetworkStack;
use super::interface::NetworkError;

use core::net::{IpAddr, SocketAddr};
use embassy_net::dns::DnsQueryType;
use embassy_net::udp::{PacketMetadata, UdpSocket};
use sntpc::{NtpContext, get_time};
use sntpc_net_embassy::UdpSocketWrapper;
use sntpc_time_embassy::EmbassyTimestampGenerator;

impl NetworkStack {
    pub async fn synchronize_time(&self, ntp_server: &str) -> Result<(), NetworkError> {
        let addresses = self.stack.dns_query(ntp_server, DnsQueryType::A).await.map_err(|_| NetworkError::Dns)?;
        let address = addresses.first().ok_or(NetworkError::Dns)?;
        let mut rx_meta = [PacketMetadata::EMPTY; 1];
        let mut rx_buffer = [0; 128];
        let mut tx_meta = [PacketMetadata::EMPTY; 1];
        let mut tx_buffer = [0; 128];
        let mut socket = UdpSocket::new(self.stack, &mut rx_meta, &mut rx_buffer, &mut tx_meta, &mut tx_buffer);

        socket.bind(0).map_err(|_| NetworkError::Socket)?;

        let socket = UdpSocketWrapper::new(socket);
        let server = SocketAddr::from((IpAddr::from(*address), 123));
        let context = NtpContext::new(EmbassyTimestampGenerator::default());
        let result = get_time(server, &socket, context).await.map_err(|_| NetworkError::Time)?;

        self.rtc.set_current_time_us(result.sec() as u64 * 1_000_000 + (result.sec_fraction() as u64 * 1_000_000) >> 32);

        Ok(())
    }
}
