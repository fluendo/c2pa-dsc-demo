use std::net::UdpSocket;

pub fn local_ip() -> String {
    let s = UdpSocket::bind("0.0.0.0:0").ok();
    if let Some(s) = s.as_ref() {
        s.connect("8.8.8.8:53").ok();
    }
    s.as_ref()
        .and_then(|s| s.local_addr().ok())
        .map(|a| a.ip().to_string())
        .unwrap_or_else(|| "127.0.0.1".to_string())
}
