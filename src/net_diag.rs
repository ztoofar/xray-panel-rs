//! Kernel TCP socket accounting for live inbound speeds.
//!
//! Xray's Linux zero-copy path (`splice`) credits a tunnel's response bytes to
//! its statistics only when the connection ends: the panel's normal stats
//! deltas read 0 B/s for the whole download and one large lump at close. The
//! kernel still tracks each socket's byte counters as it moves the data, so a
//! read-only `sock_diag` dump gives the panel a live rate without giving up
//! zero-copy.
//!
//! Scope and limits (documented in `docs/KNOWN-ISSUES.md`):
//! - Linux only; other platforms return `Unsupported`.
//! - TCP only. The counters are the socket's payload totals
//!   (`tcpi_bytes_received` from the peer, `tcpi_bytes_acked` to the peer).
//! - A socket is attributed by its local address/port, not by Xray user, so
//!   this feeds inbound speed columns only, never per-client billing.
//! - A connection that opens and closes between two samples is never seen and
//!   contributes nothing to the displayed rate.

use std::{
    collections::{HashMap, HashSet},
    io,
    net::IpAddr,
};

/// Cumulative byte counters of one live TCP socket.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SocketCounters {
    /// Kernel socket cookie; unique while the socket lives.
    pub cookie: u64,
    pub local_ip: IpAddr,
    pub local_port: u16,
    /// Bytes received from the peer (client -> Xray).
    pub up: u64,
    /// Bytes acknowledged to the peer (Xray -> client).
    pub down: u64,
}

/// Bytes a socket moved since the previous sample.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SocketDelta {
    pub local_ip: IpAddr,
    pub local_port: u16,
    pub up: u64,
    pub down: u64,
}

/// Keeps one baseline per live socket and turns dumps into per-window deltas.
///
/// The first sighting of a socket only seeds its baseline: a connection that
/// spent the last hour transferring must not dump its lifetime bytes into one
/// second of "speed".
#[derive(Default)]
pub struct SocketSampler {
    last: HashMap<u64, (u64, u64)>,
}

impl SocketSampler {
    pub fn clear(&mut self) {
        self.last.clear();
    }

    pub fn sample(&mut self, ports: &HashSet<u16>) -> io::Result<Vec<SocketDelta>> {
        if ports.is_empty() {
            self.last.clear();
            return Ok(Vec::new());
        }
        let sockets = platform::dump_tcp_sockets(ports)?;
        Ok(self.fold(sockets))
    }

    fn fold(&mut self, sockets: Vec<SocketCounters>) -> Vec<SocketDelta> {
        let mut next = HashMap::with_capacity(sockets.len());
        let mut out = Vec::with_capacity(sockets.len());
        for socket in sockets {
            let (up, down) = match self.last.remove(&socket.cookie) {
                // A counter that moved backwards means the cookie was reused
                // or the kernel restarted its count; treat it as a new socket.
                Some((last_up, last_down)) if socket.up >= last_up && socket.down >= last_down => {
                    (socket.up - last_up, socket.down - last_down)
                }
                _ => (0, 0),
            };
            next.insert(socket.cookie, (socket.up, socket.down));
            out.push(SocketDelta {
                local_ip: socket.local_ip,
                local_port: socket.local_port,
                up,
                down,
            });
        }
        // Sockets missing from this dump are gone; dropping their baselines is
        // what keeps the map bounded by the live socket count.
        self.last = next;
        out
    }
}

#[cfg(any(target_os = "linux", test))]
mod parse {
    use std::{collections::HashSet, io, net::IpAddr};

    use super::SocketCounters;

    const NLMSG_HEADER: usize = 16;
    const DIAG_MSG_HEADER: usize = 72;
    const NLMSG_ERROR: u16 = 2;
    const NLMSG_DONE: u16 = 3;
    const SOCK_DIAG_BY_FAMILY: u16 = 20;
    const INET_DIAG_INFO: u16 = 2;
    const TCP_LISTEN: u8 = 10;
    // Linux 128-bit tcp_info: bytes_acked/bytes_received sit after 24 u32
    // fields, following two u64 pacing-rate fields (8-byte header + 96 + 16).
    // Older kernels return a shorter attribute; those sockets are skipped.
    const TCP_INFO_MIN_LEN: usize = 136;
    const BYTES_ACKED: usize = 120;
    const BYTES_RECEIVED: usize = 128;

    /// Parses one netlink datagram. Returns `true` once NLMSG_DONE (or an
    /// empty NLMSG_ERROR) ends the dump.
    pub(super) fn parse_dump(
        buffer: &[u8],
        ports: &HashSet<u16>,
        out: &mut Vec<SocketCounters>,
    ) -> io::Result<bool> {
        let mut offset = 0;
        while offset + NLMSG_HEADER <= buffer.len() {
            let length =
                u32::from_ne_bytes(buffer[offset..offset + 4].try_into().unwrap()) as usize;
            let kind = u16::from_ne_bytes(buffer[offset + 4..offset + 6].try_into().unwrap());
            if length < NLMSG_HEADER || offset + length > buffer.len() {
                return Err(invalid("truncated netlink message"));
            }
            match kind {
                NLMSG_DONE => return Ok(true),
                NLMSG_ERROR if length >= NLMSG_HEADER + 4 => {
                    let code =
                        i32::from_ne_bytes(buffer[offset + 16..offset + 20].try_into().unwrap());
                    if code != 0 {
                        return Err(io::Error::from_raw_os_error(-code));
                    }
                    return Ok(true);
                }
                SOCK_DIAG_BY_FAMILY => parse_socket(&buffer[offset..offset + length], ports, out),
                _ => {}
            }
            offset += (length + 3) & !3;
        }
        Ok(false)
    }

    fn parse_socket(message: &[u8], ports: &HashSet<u16>, out: &mut Vec<SocketCounters>) {
        let payload = &message[NLMSG_HEADER..];
        if payload.len() < DIAG_MSG_HEADER {
            return;
        }
        let family = payload[0];
        let state = payload[1];
        // Listening sockets carry no traffic of their own.
        if state == TCP_LISTEN {
            return;
        }
        let local_port = u16::from_be_bytes([payload[4], payload[5]]);
        if !ports.contains(&local_port) {
            return;
        }
        let local_ip = match family {
            2 => IpAddr::V4(std::net::Ipv4Addr::new(
                payload[8],
                payload[9],
                payload[10],
                payload[11],
            )),
            10 => {
                let mut octets = [0u8; 16];
                octets.copy_from_slice(&payload[8..24]);
                IpAddr::V6(std::net::Ipv6Addr::from(octets))
            }
            _ => return,
        };

        let mut offset = DIAG_MSG_HEADER;
        while offset + 4 <= payload.len() {
            let length = u16::from_ne_bytes([payload[offset], payload[offset + 1]]) as usize;
            let kind = u16::from_ne_bytes([payload[offset + 2], payload[offset + 3]]) & 0x3fff;
            if length < 4 || offset + length > payload.len() {
                return;
            }
            if kind == INET_DIAG_INFO && length >= 4 + TCP_INFO_MIN_LEN {
                let info = &payload[offset + 4..offset + length];
                // The cookie is stored as two 32-bit halves (low, high), not
                // as a native-endian u64.
                let cookie = u64::from(u32::from_ne_bytes(payload[44..48].try_into().unwrap()))
                    | (u64::from(u32::from_ne_bytes(payload[48..52].try_into().unwrap())) << 32);
                let cookie = if cookie == 0 {
                    fallback_cookie(family, &payload[8..24], local_port)
                } else {
                    cookie
                };
                out.push(SocketCounters {
                    cookie,
                    local_ip,
                    local_port,
                    up: u64::from_ne_bytes(
                        info[BYTES_RECEIVED..BYTES_RECEIVED + 8].try_into().unwrap(),
                    ),
                    down: u64::from_ne_bytes(
                        info[BYTES_ACKED..BYTES_ACKED + 8].try_into().unwrap(),
                    ),
                });
                return;
            }
            offset += (length + 3) & !3;
        }
    }

    /// Old kernels may report an empty cookie; keep sockets distinct anyway.
    fn fallback_cookie(family: u8, address: &[u8], port: u16) -> u64 {
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        for byte in [family, port as u8, (port >> 8) as u8]
            .into_iter()
            .chain(address.iter().copied())
        {
            hash ^= u64::from(byte);
            hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
        }
        hash.max(1)
    }

    fn invalid(message: &'static str) -> io::Error {
        io::Error::new(io::ErrorKind::InvalidData, message)
    }
}

#[cfg(target_os = "linux")]
mod platform {
    use std::{
        collections::HashSet,
        io,
        mem::size_of,
        os::fd::{AsRawFd, FromRawFd, OwnedFd, RawFd},
    };

    use super::SocketCounters;

    const AF_NETLINK: i32 = 16;
    const NETLINK_SOCK_DIAG: i32 = 4;
    const NLMSG_REQUEST: u16 = 0x1;
    const NLMSG_DUMP: u16 = 0x300;
    const REQUEST_LEN: usize = 16 + 56;
    const AF_INET: u8 = 2;
    const AF_INET6: u8 = 10;
    const IPPROTO_TCP: u8 = 6;
    const INET_DIAG_INFO_BIT: u8 = 2;
    const RECEIVE_BUFFER: usize = 64 * 1024;

    #[repr(C)]
    struct SockaddrNl {
        family: u16,
        pad: u16,
        pid: u32,
        groups: u32,
    }

    pub(super) fn dump_tcp_sockets(ports: &HashSet<u16>) -> io::Result<Vec<SocketCounters>> {
        let socket = open_socket()?;
        let mut out = Vec::new();
        for family in [AF_INET, AF_INET6] {
            send_dump(socket.as_raw_fd(), family)?;
            read_dump(socket.as_raw_fd(), ports, &mut out)?;
        }
        Ok(out)
    }

    fn open_socket() -> io::Result<OwnedFd> {
        let fd = unsafe {
            libc::socket(
                AF_NETLINK,
                libc::SOCK_RAW | libc::SOCK_CLOEXEC,
                NETLINK_SOCK_DIAG,
            )
        };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        let fd = unsafe { OwnedFd::from_raw_fd(fd) };
        let address = netlink_address();
        let bound = unsafe {
            libc::bind(
                fd.as_raw_fd(),
                &address as *const SockaddrNl as *const libc::sockaddr,
                size_of::<SockaddrNl>() as libc::socklen_t,
            )
        };
        if bound < 0 {
            return Err(io::Error::last_os_error());
        }
        // A kernel dump that stops answering must not stall the display tick.
        let timeout = libc::timeval {
            tv_sec: 1,
            tv_usec: 0,
        };
        let set = unsafe {
            libc::setsockopt(
                fd.as_raw_fd(),
                libc::SOL_SOCKET,
                libc::SO_RCVTIMEO,
                &timeout as *const libc::timeval as *const libc::c_void,
                size_of::<libc::timeval>() as libc::socklen_t,
            )
        };
        if set < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(fd)
    }

    fn netlink_address() -> SockaddrNl {
        SockaddrNl {
            family: AF_NETLINK as u16,
            pad: 0,
            pid: 0,
            groups: 0,
        }
    }

    fn send_dump(fd: RawFd, family: u8) -> io::Result<()> {
        let mut request = [0u8; REQUEST_LEN];
        request[0..4].copy_from_slice(&(REQUEST_LEN as u32).to_ne_bytes());
        request[4..6].copy_from_slice(&20u16.to_ne_bytes()); // SOCK_DIAG_BY_FAMILY
        request[6..8].copy_from_slice(&(NLMSG_REQUEST | NLMSG_DUMP).to_ne_bytes());
        request[16] = family;
        request[17] = IPPROTO_TCP;
        request[18] = 1 << (INET_DIAG_INFO_BIT - 1);
        request[20..24].copy_from_slice(&u32::MAX.to_ne_bytes());
        let destination = netlink_address();
        let sent = unsafe {
            libc::sendto(
                fd,
                request.as_ptr() as *const libc::c_void,
                request.len(),
                0,
                &destination as *const SockaddrNl as *const libc::sockaddr,
                size_of::<SockaddrNl>() as libc::socklen_t,
            )
        };
        if sent < 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(())
    }

    fn read_dump(fd: RawFd, ports: &HashSet<u16>, out: &mut Vec<SocketCounters>) -> io::Result<()> {
        let mut buffer = vec![0u8; RECEIVE_BUFFER];
        loop {
            let received = unsafe {
                libc::recv(
                    fd,
                    buffer.as_mut_ptr() as *mut libc::c_void,
                    buffer.len(),
                    0,
                )
            };
            if received < 0 {
                return Err(io::Error::last_os_error());
            }
            if received == 0 {
                return Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "netlink dump ended without NLMSG_DONE",
                ));
            }
            if super::parse::parse_dump(&buffer[..received as usize], ports, out)? {
                return Ok(());
            }
        }
    }
}

#[cfg(not(target_os = "linux"))]
mod platform {
    use std::{collections::HashSet, io};

    use super::SocketCounters;

    pub(super) fn dump_tcp_sockets(_ports: &HashSet<u16>) -> io::Result<Vec<SocketCounters>> {
        Err(io::Error::new(
            io::ErrorKind::Unsupported,
            "kernel socket statistics need Linux",
        ))
    }
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr};

    use super::{parse::parse_dump, SocketCounters, SocketSampler};

    fn counters(cookie: u64, up: u64, down: u64) -> SocketCounters {
        SocketCounters {
            cookie,
            local_ip: IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)),
            local_port: 8443,
            up,
            down,
        }
    }

    #[test]
    fn first_sighting_seeds_the_baseline() {
        let mut sampler = SocketSampler::default();
        let deltas = sampler.fold(vec![counters(1, 1_000, 9_000)]);
        assert_eq!(deltas.len(), 1);
        assert_eq!((deltas[0].up, deltas[0].down), (0, 0));
    }

    #[test]
    fn reports_only_the_growth_since_the_last_sample() {
        let mut sampler = SocketSampler::default();
        sampler.fold(vec![counters(1, 1_000, 9_000)]);
        let deltas = sampler.fold(vec![counters(1, 1_500, 10_000)]);
        assert_eq!((deltas[0].up, deltas[0].down), (500, 1_000));
    }

    #[test]
    fn closed_sockets_leave_the_baseline() {
        let mut sampler = SocketSampler::default();
        sampler.fold(vec![counters(1, 1_000, 9_000), counters(2, 10, 20)]);
        // Socket 2 closed; socket 3 was created with counters lower than a
        // stale entry for the same cookie would have been.
        let deltas = sampler.fold(vec![counters(1, 1_000, 9_500)]);
        assert_eq!(deltas.len(), 1);
        assert_eq!(deltas[0].down, 500);
        assert_eq!(sampler.fold(vec![counters(3, 5, 5)])[0].down, 0);
    }

    #[test]
    fn a_backwards_counter_is_treated_as_a_new_socket() {
        let mut sampler = SocketSampler::default();
        sampler.fold(vec![counters(7, 5_000, 50_000)]);
        let deltas = sampler.fold(vec![counters(7, 100, 200)]);
        assert_eq!((deltas[0].up, deltas[0].down), (0, 0));
        // And the new baseline is the smaller value.
        let deltas = sampler.fold(vec![counters(7, 150, 300)]);
        assert_eq!((deltas[0].up, deltas[0].down), (50, 100));
    }

    // Build a SOCK_DIAG_BY_FAMILY reply with one ESTABLISHED v4 socket.
    fn diag_message(port: u16, cookie: u64, up: u64, down: u64) -> Vec<u8> {
        let mut message = vec![0u8; 16 + 72];
        let header_len = message.len() as u32;
        message[0..4].copy_from_slice(&header_len.to_ne_bytes());
        message[4..6].copy_from_slice(&20u16.to_ne_bytes());
        message[16] = 2; // AF_INET
        message[17] = 1; // TCP_ESTABLISHED
        message[20..22].copy_from_slice(&port.to_be_bytes());
        message[24..28].copy_from_slice(&[127, 0, 0, 1]); // idiag_src
        message[60..64].copy_from_slice(&((cookie & 0xffff_ffff) as u32).to_ne_bytes());
        message[64..68].copy_from_slice(&((cookie >> 32) as u32).to_ne_bytes());

        let mut info = vec![0u8; 136];
        // Offsets are relative to the tcp_info payload, not the attribute.
        info[120..128].copy_from_slice(&down.to_ne_bytes());
        info[128..136].copy_from_slice(&up.to_ne_bytes());
        let mut attribute = vec![0u8; 4];
        attribute.extend_from_slice(&info);
        let length = attribute.len() as u16;
        attribute[0..2].copy_from_slice(&length.to_ne_bytes());
        attribute[2..4].copy_from_slice(&2u16.to_ne_bytes()); // INET_DIAG_INFO
        message.extend_from_slice(&attribute);

        let length = message.len() as u32;
        message[0..4].copy_from_slice(&length.to_ne_bytes());
        message
    }

    fn done_message() -> Vec<u8> {
        let mut message = vec![0u8; 16];
        message[0..4].copy_from_slice(&16u32.to_ne_bytes());
        message[4..6].copy_from_slice(&3u16.to_ne_bytes()); // NLMSG_DONE
        message
    }

    #[test]
    fn parses_socket_byte_counters_from_a_netlink_dump() {
        let mut buffer = diag_message(8443, 0x1_0000_0002, 500, 1_000);
        buffer.extend_from_slice(&done_message());
        let ports = [8443u16].into_iter().collect();
        let mut out = Vec::new();
        assert!(parse_dump(&buffer, &ports, &mut out).unwrap());
        assert_eq!(out.len(), 1);
        assert_eq!(out[0].cookie, 0x1_0000_0002);
        assert_eq!(out[0].local_port, 8443);
        assert_eq!(out[0].local_ip, IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1)));
        assert_eq!((out[0].up, out[0].down), (500, 1_000));
    }

    #[test]
    fn ignores_ports_the_caller_did_not_ask_for() {
        let buffer = diag_message(9000, 1, 500, 1_000);
        let ports = [8443u16].into_iter().collect();
        let mut out = Vec::new();
        assert!(!parse_dump(&buffer, &ports, &mut out).unwrap());
        assert!(out.is_empty());
    }

    #[test]
    fn a_truncated_message_is_an_error_not_a_panic() {
        let ports = [8443u16].into_iter().collect();
        let mut out = Vec::new();
        assert!(parse_dump(&[0u8; 8], &ports, &mut out).is_ok());
        let mut message = diag_message(8443, 1, 500, 1_000);
        message[0..4].copy_from_slice(&10_000u32.to_ne_bytes());
        assert!(parse_dump(&message[..20], &ports, &mut out).is_err());
    }
}
