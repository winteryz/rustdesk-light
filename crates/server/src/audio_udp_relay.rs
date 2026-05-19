use rdl_protocol::audio_udp;
use std::collections::HashMap;
use std::io;
use std::net::{SocketAddr, UdpSocket};
use std::thread;
use std::time::{Duration, Instant};

const IDLE_TIMEOUT_MS: u64 = 30_000;
const RECV_TIMEOUT_MS: u64 = 100;
const MAINTENANCE_MS: u64 = 1_000;

#[derive(Clone, Copy)]
struct AudioUdpRoute {
    receiver_addr: SocketAddr,
    last_seen: Instant,
}

pub(crate) fn start(bind_addr: String) {
    thread::spawn(move || match UdpSocket::bind(&bind_addr) {
        Ok(socket) => {
            println!("audio udp relay listening on {bind_addr}");
            if let Err(error) = relay_loop(socket) {
                eprintln!("audio udp relay stopped: {error}");
            }
        }
        Err(error) => eprintln!("audio udp relay bind failed on {bind_addr}: {error}"),
    });
}

fn relay_loop(socket: UdpSocket) -> io::Result<()> {
    socket.set_read_timeout(Some(Duration::from_millis(RECV_TIMEOUT_MS)))?;
    let mut routes = HashMap::<u64, AudioUdpRoute>::new();
    let mut buf = [0_u8; audio_udp::MAX_PACKET_BYTES];
    let mut last_maintenance = Instant::now();
    loop {
        match socket.recv_from(&mut buf) {
            Ok((len, addr)) => match audio_udp::decode(&buf[..len]) {
                Ok(audio_udp::Packet::Register { stream_id }) => {
                    routes.insert(
                        stream_id,
                        AudioUdpRoute {
                            receiver_addr: addr,
                            last_seen: Instant::now(),
                        },
                    );
                }
                Ok(audio_udp::Packet::Unregister { stream_id }) => {
                    if routes
                        .get(&stream_id)
                        .map(|route| route.receiver_addr == addr)
                        .unwrap_or(false)
                    {
                        routes.remove(&stream_id);
                    }
                }
                Ok(audio_udp::Packet::Audio { stream_id, .. }) => {
                    if let Some(route) = routes.get_mut(&stream_id) {
                        route.last_seen = Instant::now();
                        let _ = socket.send_to(&buf[..len], route.receiver_addr);
                    }
                }
                Err(error) => {
                    eprintln!("audio udp relay ignored packet from {addr}: {error}");
                }
            },
            Err(error)
                if error.kind() == io::ErrorKind::WouldBlock
                    || error.kind() == io::ErrorKind::TimedOut => {}
            Err(error) => return Err(error),
        }

        if last_maintenance.elapsed() >= Duration::from_millis(MAINTENANCE_MS) {
            let now = Instant::now();
            routes.retain(|_, route| {
                now.duration_since(route.last_seen) < Duration::from_millis(IDLE_TIMEOUT_MS)
            });
            last_maintenance = now;
        }
    }
}
