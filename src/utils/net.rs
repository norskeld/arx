use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpStream};
use std::time::Duration;

#[allow(dead_code)]
pub fn is_online() -> bool {
  let ip = Ipv4Addr::new(1, 1, 1, 1);
  let address = SocketAddr::V4(SocketAddrV4::new(ip, 80));

  TcpStream::connect_timeout(&address, Duration::from_secs(5)).map_or(false, |_| true)
}
