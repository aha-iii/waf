//! Edge listen: optional SO_REUSEPORT multi-process shared port (Linux).

use std::net::SocketAddr;

pub async fn bind_listener(addr: SocketAddr) -> std::io::Result<tokio::net::TcpListener> {
    let reuse = matches!(
        std::env::var("NERVE_EDGE_REUSEPORT").as_deref(),
        Ok("1") | Ok("true") | Ok("yes")
    );
    #[cfg(unix)]
    if reuse {
        return bind_reuseport_unix(addr).await;
    }
    tokio::net::TcpListener::bind(addr).await
}

#[cfg(unix)]
async fn bind_reuseport_unix(addr: SocketAddr) -> std::io::Result<tokio::net::TcpListener> {
    use socket2::{Domain, Socket, Type};
    use std::os::fd::{FromRawFd, IntoRawFd};

    let domain = if addr.is_ipv4() {
        Domain::IPV4
    } else {
        Domain::IPV6
    };
    let socket = Socket::new(domain, Type::STREAM, None)?;
    socket.set_reuse_address(true)?;
    #[cfg(all(unix, not(target_os = "solaris")))]
    socket.set_reuse_port(true)?;
    socket.set_nonblocking(true)?;
    socket.bind(&addr.into())?;
    socket.listen(1024)?;
    let fd = socket.into_raw_fd();
    let std_listener = unsafe { std::net::TcpListener::from_raw_fd(fd) };
    tokio::net::TcpListener::from_std(std_listener)
}

#[cfg(not(unix))]
async fn bind_reuseport_unix(_addr: SocketAddr) -> std::io::Result<tokio::net::TcpListener> {
    unreachable!("reuseport only on unix cfg branch")
}
