use super::*;
use rdl_protocol::audio_udp as protocol_audio_udp;
use std::net::UdpSocket;

const AUDIO_UDP_REGISTER_INTERVAL_MS: u64 = 250;
const AUDIO_UDP_RECV_TIMEOUT_MS: u64 = 20;
const AUDIO_STREAM_REPORT_INTERVAL_MS: u64 = 1_000;
const MAX_PENDING_AUDIO_MS: u64 = 240;
const MAX_PENDING_AUDIO_FRAMES_PER_SOURCE: usize = 32;

pub(super) struct PendingAudioFrame {
    pub(super) source: AudioSource,
    pub(super) seq: u64,
    pub(super) sample_rate: u32,
    pub(super) channels: u16,
    pub(super) format: String,
    pub(super) bytes: Vec<u8>,
}

pub(super) struct AudioUdpSession {
    pub(super) stream_id: u64,
    stop: Arc<AtomicBool>,
}

#[derive(Clone)]
pub(super) struct AudioUdpEndpoint {
    pub(super) host: String,
    pub(super) port: u16,
    pub(super) stream_id: u64,
}

pub(super) struct AudioUdpSender {
    socket: UdpSocket,
    stream_id: u64,
    packet: Vec<u8>,
    sent_packets: u64,
    sent_bytes: u64,
    last_report: Instant,
}

impl AudioUdpSender {
    pub(super) fn connect(endpoint: &AudioUdpEndpoint) -> Result<Self, String> {
        let socket =
            UdpSocket::bind("0.0.0.0:0").map_err(|error| format!("bind udp failed: {error}"))?;
        socket
            .connect(endpoint.addr())
            .map_err(|error| format!("connect udp relay failed: {error}"))?;
        Ok(Self {
            socket,
            stream_id: endpoint.stream_id,
            packet: Vec::with_capacity(protocol_audio_udp::MAX_PACKET_BYTES),
            sent_packets: 0,
            sent_bytes: 0,
            last_report: Instant::now(),
        })
    }

    pub(super) fn send_frame(
        &mut self,
        client_id: &str,
        seq: u64,
        sample_rate: u32,
        channels: u16,
        format: &str,
        bytes: &[u8],
    ) -> io::Result<()> {
        protocol_audio_udp::encode_audio(
            self.stream_id,
            seq,
            rdl_protocol::now_epoch_ms() as u64,
            sample_rate,
            channels,
            format,
            bytes,
            &mut self.packet,
        )
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
        self.socket.send(&self.packet)?;
        self.sent_packets = self.sent_packets.saturating_add(1);
        self.sent_bytes = self.sent_bytes.saturating_add(bytes.len() as u64);
        if self.last_report.elapsed() >= Duration::from_millis(AUDIO_STREAM_REPORT_INTERVAL_MS) {
            debug_log!(
                "debug event=voice_chat_tx client={} transport=udp packets={} bytes={}",
                client_id,
                self.sent_packets,
                self.sent_bytes
            );
            self.last_report = Instant::now();
        }
        Ok(())
    }
}

impl AudioUdpEndpoint {
    pub(super) fn from_payload(payload: &str) -> Result<Option<Self>, String> {
        if payload_field(payload, "transport").as_deref() != Some("udp") {
            return Ok(None);
        }
        let host = payload_field(payload, "udp_host")
            .ok_or_else(|| "missing audio udp host".to_string())?;
        let port = payload_field(payload, "udp_port")
            .and_then(|value| value.parse::<u16>().ok())
            .ok_or_else(|| "missing audio udp port".to_string())?;
        let stream_id = payload_field(payload, "udp_stream")
            .and_then(|value| value.parse::<u64>().ok())
            .ok_or_else(|| "missing audio udp stream".to_string())?;
        Ok(Some(Self {
            host,
            port,
            stream_id,
        }))
    }

    fn addr(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

pub(super) fn initial_stream_id() -> u64 {
    rdl_protocol::now_epoch_ms() as u64
}

pub(super) fn push_pending_audio_frame(
    queues: &mut HashMap<(String, u8), VecDeque<PendingAudioFrame>>,
    client_id: String,
    frame: PendingAudioFrame,
) {
    let source_key = audio_source_key(&frame.source);
    let queue = queues.entry((client_id, source_key)).or_default();
    queue.push_back(frame);
    while queue.len() > MAX_PENDING_AUDIO_FRAMES_PER_SOURCE
        || pending_audio_duration_ms(queue) > MAX_PENDING_AUDIO_MS
    {
        if queue.len() <= 1 {
            break;
        }
        let _ = queue.pop_front();
    }
}

fn pending_audio_duration_ms(queue: &VecDeque<PendingAudioFrame>) -> u64 {
    queue.iter().map(pending_audio_frame_duration_ms).sum()
}

fn pending_audio_frame_duration_ms(frame: &PendingAudioFrame) -> u64 {
    let channels = frame.channels.max(1) as usize;
    let sample_rate = frame.sample_rate.max(1) as u64;
    let frames = frame.bytes.len() / 2 / channels;
    ((frames as u64 * 1000) / sample_rate).max(1)
}

fn audio_source_key(source: &AudioSource) -> u8 {
    match source {
        AudioSource::AudioListen => 1,
        AudioSource::VoiceChat => 2,
    }
}

pub(super) fn voice_audio_forward_loop(
    voice_audio_rx: Receiver<user_interaction::voice_chat::OutboundCommand>,
    voice_udp_senders: Arc<Mutex<HashMap<String, AudioUdpSender>>>,
    voice_udp_endpoints: Arc<Mutex<HashMap<String, AudioUdpEndpoint>>>,
) {
    let mut missing_senders = HashMap::<String, (u64, Instant)>::new();
    let mut send_failures = HashMap::<String, (u64, Instant, String)>::new();
    while let Ok(command) = voice_audio_rx.recv() {
        let user_interaction::voice_chat::OutboundCommand::AudioFrame {
            client_id,
            seq,
            sample_rate,
            channels,
            format,
            bytes,
        } = command
        else {
            continue;
        };
        let mut remove_sender = false;
        let endpoint = voice_udp_endpoints
            .lock()
            .ok()
            .and_then(|endpoints| endpoints.get(&client_id).cloned());
        if let Ok(mut senders) = voice_udp_senders.lock() {
            if !senders.contains_key(&client_id) {
                if let Some(endpoint) = endpoint.as_ref() {
                    match AudioUdpSender::connect(endpoint) {
                        Ok(sender) => {
                            debug_log!(
                                "debug event=voice_chat_udp_sender_recovered client={} stream={}",
                                client_id,
                                endpoint.stream_id
                            );
                            senders.insert(client_id.clone(), sender);
                        }
                        Err(error) => {
                            debug_log!(
                                "debug event=voice_chat_udp_sender_recover_failed client={} error={}",
                                client_id, error
                            );
                        }
                    }
                }
            }
            if let Some(sender) = senders.get_mut(&client_id) {
                missing_senders.remove(&client_id);
                if let Err(error) =
                    sender.send_frame(&client_id, seq, sample_rate, channels, &format, &bytes)
                {
                    report_voice_udp_send_failure(&mut send_failures, &client_id, &error);
                    remove_sender = error.kind() != io::ErrorKind::InvalidInput;
                } else {
                    send_failures.remove(&client_id);
                }
            }
            if remove_sender {
                senders.remove(&client_id);
            } else if !senders.contains_key(&client_id) {
                let entry = missing_senders
                    .entry(client_id.clone())
                    .or_insert((0, Instant::now()));
                entry.0 = entry.0.saturating_add(1);
                if entry.1.elapsed() >= Duration::from_millis(AUDIO_STREAM_REPORT_INTERVAL_MS) {
                    debug_log!(
                        "debug event=voice_chat_udp_missing_sender client={} frames={}",
                        client_id,
                        entry.0
                    );
                    entry.1 = Instant::now();
                }
            }
        }
    }
}

fn report_voice_udp_send_failure(
    send_failures: &mut HashMap<String, (u64, Instant, String)>,
    client_id: &str,
    error: &io::Error,
) {
    let error_text = error.to_string();
    let entry = send_failures
        .entry(client_id.to_string())
        .or_insert_with(|| {
            (
                0,
                Instant::now() - Duration::from_millis(AUDIO_STREAM_REPORT_INTERVAL_MS),
                error_text.clone(),
            )
        });
    if entry.2 != error_text {
        entry.0 = 0;
        entry.1 = Instant::now() - Duration::from_millis(AUDIO_STREAM_REPORT_INTERVAL_MS);
        entry.2 = error_text;
    }
    entry.0 = entry.0.saturating_add(1);
    if entry.1.elapsed() >= Duration::from_millis(AUDIO_STREAM_REPORT_INTERVAL_MS) {
        debug_log!(
            "debug event=voice_chat_udp_send_failed client={} count={} error={}",
            client_id,
            entry.0,
            entry.2
        );
        entry.0 = 0;
        entry.1 = Instant::now();
    }
}

fn audio_udp_receive_loop(
    socket: UdpSocket,
    server_addr: String,
    client_id: String,
    source: AudioSource,
    stream_id: u64,
    stop: Arc<AtomicBool>,
    event_sink: AdminEventSink,
) {
    let mut register_packet = Vec::new();
    let mut unregister_packet = Vec::new();
    protocol_audio_udp::encode_register(stream_id, &mut register_packet);
    protocol_audio_udp::encode_unregister(stream_id, &mut unregister_packet);
    let mut last_register = Instant::now() - Duration::from_millis(AUDIO_UDP_REGISTER_INTERVAL_MS);
    let mut buf = [0_u8; protocol_audio_udp::MAX_PACKET_BYTES];

    while !stop.load(Ordering::Relaxed) {
        if last_register.elapsed() >= Duration::from_millis(AUDIO_UDP_REGISTER_INTERVAL_MS) {
            if let Err(error) = socket.send_to(&register_packet, &server_addr) {
                event_sink.send(AdminEvent::Log(format!(
                    "audio udp register failed: {error}"
                )));
                break;
            }
            last_register = Instant::now();
        }

        match socket.recv_from(&mut buf) {
            Ok((len, _)) => match protocol_audio_udp::decode(&buf[..len]) {
                Ok(protocol_audio_udp::Packet::Audio {
                    stream_id: packet_stream_id,
                    seq,
                    sample_rate,
                    channels,
                    format,
                    bytes,
                    ..
                }) if packet_stream_id == stream_id => {
                    event_sink.send(AdminEvent::AudioFrame {
                        client_id: client_id.clone(),
                        source: source.clone(),
                        seq,
                        sample_rate,
                        channels,
                        format: format.to_string(),
                        bytes: bytes.to_vec(),
                    });
                }
                Ok(_) => {}
                Err(error) => {
                    event_sink.send(AdminEvent::Log(format!(
                        "audio udp packet ignored: {error}"
                    )));
                }
            },
            Err(error)
                if error.kind() == io::ErrorKind::WouldBlock
                    || error.kind() == io::ErrorKind::TimedOut => {}
            Err(error) => {
                event_sink.send(AdminEvent::Log(format!(
                    "audio udp receive failed: {error}"
                )));
                break;
            }
        }
    }

    let _ = socket.send_to(&unregister_packet, server_addr);
}

impl AdminApp {
    pub(super) fn next_audio_udp_stream_id(&mut self) -> u64 {
        let stream_id = self.audio_udp_next_stream_id.max(1);
        self.audio_udp_next_stream_id = self.audio_udp_next_stream_id.saturating_add(1);
        stream_id
    }

    fn start_audio_udp_receive_session(
        &mut self,
        client_id: &str,
        source: AudioSource,
    ) -> Option<AudioUdpSession> {
        let server_addr = format!("{}:{}", self.config.ip, self.config.port);
        let socket = match UdpSocket::bind("0.0.0.0:0") {
            Ok(socket) => socket,
            Err(error) => {
                self.push_log(format!("audio udp bind failed: {error}"));
                return None;
            }
        };
        if let Err(error) =
            socket.set_read_timeout(Some(Duration::from_millis(AUDIO_UDP_RECV_TIMEOUT_MS)))
        {
            self.push_log(format!("audio udp timeout setup failed: {error}"));
            return None;
        }
        let stream_id = self.next_audio_udp_stream_id();
        let stop = Arc::new(AtomicBool::new(false));
        let event_sink = AdminEventSink::new(
            self.event_tx.clone(),
            Some(self.repaint_handle.clone()),
            Some(self.audio_playback_registry.clone()),
        );
        let worker_stop = stop.clone();
        let worker_client_id = client_id.to_string();
        let worker_server_addr = server_addr.clone();
        let worker_source = source.clone();
        thread::spawn(move || {
            audio_udp_receive_loop(
                socket,
                worker_server_addr,
                worker_client_id,
                worker_source,
                stream_id,
                worker_stop,
                event_sink,
            );
        });
        self.push_log(format!(
            "audio udp session started client={client_id} source={} stream={stream_id} relay={server_addr}",
            source.as_str()
        ));
        Some(AudioUdpSession { stream_id, stop })
    }

    pub(super) fn start_audio_udp_session(&mut self, client_id: &str) -> Option<u64> {
        self.stop_audio_udp_session(client_id);
        let session = self.start_audio_udp_receive_session(client_id, AudioSource::AudioListen)?;
        let stream_id = session.stream_id;
        self.audio_udp_sessions
            .insert(client_id.to_string(), session);
        Some(stream_id)
    }

    pub(super) fn stop_audio_udp_session(&mut self, client_id: &str) {
        if let Some(session) = self.audio_udp_sessions.remove(client_id) {
            session.stop.store(true, Ordering::Relaxed);
            self.push_log(format!(
                "audio udp session stopped client={client_id} stream={}",
                session.stream_id
            ));
        }
    }

    pub(super) fn stop_all_audio_udp_sessions(&mut self) {
        let client_ids: Vec<String> = self.audio_udp_sessions.keys().cloned().collect();
        for client_id in client_ids {
            self.stop_audio_udp_session(&client_id);
        }
    }

    pub(super) fn start_voice_udp_session(&mut self, client_id: &str) -> Option<u64> {
        self.stop_voice_udp_session(client_id);
        let session = self.start_audio_udp_receive_session(client_id, AudioSource::VoiceChat)?;
        let stream_id = session.stream_id;
        self.voice_udp_sessions
            .insert(client_id.to_string(), session);
        Some(stream_id)
    }

    pub(super) fn stop_voice_udp_session(&mut self, client_id: &str) {
        if let Some(session) = self.voice_udp_sessions.remove(client_id) {
            session.stop.store(true, Ordering::Relaxed);
            self.push_log(format!(
                "voice udp session stopped client={client_id} stream={}",
                session.stream_id
            ));
        }
    }

    pub(super) fn stop_all_voice_udp_sessions(&mut self) {
        let client_ids: Vec<String> = self.voice_udp_sessions.keys().cloned().collect();
        for client_id in client_ids {
            self.stop_voice_udp_session(&client_id);
        }
    }

    pub(super) fn set_voice_udp_sender(
        &mut self,
        client_id: &str,
        detail: &str,
    ) -> Result<(), String> {
        let endpoint = AudioUdpEndpoint::from_payload(detail)?
            .ok_or_else(|| "voice chat udp transport unavailable".to_string())?;
        self.set_voice_udp_sender_endpoint(client_id, &endpoint)
    }

    pub(super) fn set_voice_udp_sender_endpoint(
        &mut self,
        client_id: &str,
        endpoint: &AudioUdpEndpoint,
    ) -> Result<(), String> {
        let sender = AudioUdpSender::connect(endpoint)?;
        self.voice_udp_endpoints
            .lock()
            .map_err(|_| "voice udp endpoint map is poisoned".to_string())?
            .insert(client_id.to_string(), endpoint.clone());
        self.voice_udp_senders
            .lock()
            .map_err(|_| "voice udp sender map is poisoned".to_string())?
            .insert(client_id.to_string(), sender);
        self.push_log(format!(
            "voice udp sender ready client={client_id} stream={} relay={}:{}",
            endpoint.stream_id, endpoint.host, endpoint.port
        ));
        Ok(())
    }

    pub(super) fn remove_voice_udp_sender(&mut self, client_id: &str) {
        if let Ok(mut senders) = self.voice_udp_senders.lock() {
            senders.remove(client_id);
        }
        if let Ok(mut endpoints) = self.voice_udp_endpoints.lock() {
            endpoints.remove(client_id);
        }
    }

    pub(super) fn clear_voice_udp_senders(&mut self) {
        if let Ok(mut senders) = self.voice_udp_senders.lock() {
            senders.clear();
        }
        if let Ok(mut endpoints) = self.voice_udp_endpoints.lock() {
            endpoints.clear();
        }
    }
}
