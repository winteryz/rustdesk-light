use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use std::collections::VecDeque;
use std::env;
use std::net::UdpSocket;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    mpsc::{self, Receiver, SyncSender},
    Arc, Mutex,
};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const MAGIC: &[u8; 4] = b"RDA1";
const HEADER_LEN: usize = 4 + 8 + 8 + 4 + 2;
const MAX_PACKET_BYTES: usize = 1200;
const TX_QUEUE_CAPACITY: usize = 128;
const DEFAULT_FRAME_MS: u32 = 10;
const RX_PREBUFFER_MS: usize = 40;
const RX_MAX_BUFFER_MS: usize = 200;

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        usage();
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("tx") => {
            let target = args
                .next()
                .ok_or_else(|| "missing receiver address".to_string())?;
            let frame_ms = args
                .next()
                .and_then(|value| value.parse::<u32>().ok())
                .unwrap_or(DEFAULT_FRAME_MS);
            run_tx(&target, frame_ms.max(1))
        }
        Some("rx") => {
            let bind = args.next().unwrap_or_else(|| "0.0.0.0:5200".to_string());
            run_rx(&bind)
        }
        _ => Err("missing mode".to_string()),
    }
}

fn usage() {
    eprintln!(
        "usage:\n  rdl-audio-probe rx [bind_addr]\n  rdl-audio-probe tx <receiver_addr> [frame_ms]\n\nexamples:\n  rdl-audio-probe rx 0.0.0.0:5200\n  rdl-audio-probe tx 192.168.1.23:5200 10"
    );
}

fn run_tx(target: &str, frame_ms: u32) -> Result<(), String> {
    let (sample_tx, sample_rx) = mpsc::sync_channel(TX_QUEUE_CAPACITY);
    let dropped_callbacks = Arc::new(AtomicU64::new(0));
    let input = start_input_stream(sample_tx, dropped_callbacks.clone())?;
    let socket =
        UdpSocket::bind("0.0.0.0:0").map_err(|error| format!("bind udp failed: {error}"))?;
    socket
        .connect(target)
        .map_err(|error| format!("connect udp target failed: {error}"))?;
    println!(
        "tx target={target} sample_rate={} channels={} frame_ms={frame_ms}",
        input.sample_rate, input.channels
    );
    send_loop(
        socket,
        sample_rx,
        input.sample_rate,
        frame_ms,
        dropped_callbacks,
    )
}

fn run_rx(bind: &str) -> Result<(), String> {
    let socket = UdpSocket::bind(bind).map_err(|error| format!("bind udp failed: {error}"))?;
    socket
        .set_read_timeout(Some(Duration::from_millis(100)))
        .map_err(|error| format!("set read timeout failed: {error}"))?;
    let player = AudioPlayer::start()?;
    println!("rx bind={bind}");
    recv_loop(socket, player)
}

struct AudioInputStream {
    sample_rate: u32,
    channels: u16,
    _stream: cpal::Stream,
}

fn start_input_stream(
    sample_tx: SyncSender<Vec<i16>>,
    dropped_callbacks: Arc<AtomicU64>,
) -> Result<AudioInputStream, String> {
    let host = cpal::default_host();
    let device = host
        .default_input_device()
        .ok_or_else(|| "no default input device found".to_string())?;
    let config = device
        .default_input_config()
        .map_err(|error| format!("default input config failed: {error}"))?;
    let sample_format = config.sample_format();
    let config = config.config();
    let sample_rate = config.sample_rate.0;
    let channels = config.channels;
    let stream = match sample_format {
        cpal::SampleFormat::F32 => {
            build_f32_input_stream(&device, &config, sample_tx, dropped_callbacks)
        }
        cpal::SampleFormat::I16 => {
            build_i16_input_stream(&device, &config, sample_tx, dropped_callbacks)
        }
        cpal::SampleFormat::U16 => {
            build_u16_input_stream(&device, &config, sample_tx, dropped_callbacks)
        }
        other => Err(format!("unsupported input sample format: {other:?}")),
    }?;
    stream
        .play()
        .map_err(|error| format!("start input stream failed: {error}"))?;
    Ok(AudioInputStream {
        sample_rate,
        channels,
        _stream: stream,
    })
}

fn build_f32_input_stream(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    sample_tx: SyncSender<Vec<i16>>,
    dropped_callbacks: Arc<AtomicU64>,
) -> Result<cpal::Stream, String> {
    let channels = config.channels.max(1) as usize;
    device
        .build_input_stream(
            config,
            move |data: &[f32], _| {
                send_mono_samples(
                    &sample_tx,
                    &dropped_callbacks,
                    f32_to_mono_i16(data, channels),
                );
            },
            |error| eprintln!("input stream error: {error}"),
            None,
        )
        .map_err(|error| format!("build input stream failed: {error}"))
}

fn build_i16_input_stream(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    sample_tx: SyncSender<Vec<i16>>,
    dropped_callbacks: Arc<AtomicU64>,
) -> Result<cpal::Stream, String> {
    let channels = config.channels.max(1) as usize;
    device
        .build_input_stream(
            config,
            move |data: &[i16], _| {
                send_mono_samples(
                    &sample_tx,
                    &dropped_callbacks,
                    i16_to_mono_i16(data, channels),
                );
            },
            |error| eprintln!("input stream error: {error}"),
            None,
        )
        .map_err(|error| format!("build input stream failed: {error}"))
}

fn build_u16_input_stream(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    sample_tx: SyncSender<Vec<i16>>,
    dropped_callbacks: Arc<AtomicU64>,
) -> Result<cpal::Stream, String> {
    let channels = config.channels.max(1) as usize;
    device
        .build_input_stream(
            config,
            move |data: &[u16], _| {
                send_mono_samples(
                    &sample_tx,
                    &dropped_callbacks,
                    u16_to_mono_i16(data, channels),
                );
            },
            |error| eprintln!("input stream error: {error}"),
            None,
        )
        .map_err(|error| format!("build input stream failed: {error}"))
}

fn send_mono_samples(
    sample_tx: &SyncSender<Vec<i16>>,
    dropped_callbacks: &AtomicU64,
    samples: Vec<i16>,
) {
    if samples.is_empty() {
        return;
    }
    if sample_tx.try_send(samples).is_err() {
        dropped_callbacks.fetch_add(1, Ordering::Relaxed);
    }
}

fn f32_to_mono_i16(data: &[f32], channels: usize) -> Vec<i16> {
    data.chunks_exact(channels)
        .map(|frame| {
            let sample = frame.iter().copied().sum::<f32>() / channels as f32;
            (sample.clamp(-1.0, 1.0) * i16::MAX as f32).round() as i16
        })
        .collect()
}

fn i16_to_mono_i16(data: &[i16], channels: usize) -> Vec<i16> {
    data.chunks_exact(channels)
        .map(|frame| {
            let sum: i32 = frame.iter().map(|sample| *sample as i32).sum();
            (sum / channels as i32).clamp(i16::MIN as i32, i16::MAX as i32) as i16
        })
        .collect()
}

fn u16_to_mono_i16(data: &[u16], channels: usize) -> Vec<i16> {
    data.chunks_exact(channels)
        .map(|frame| {
            let sum: i32 = frame.iter().map(|sample| *sample as i32 - 32768).sum();
            (sum / channels as i32).clamp(i16::MIN as i32, i16::MAX as i32) as i16
        })
        .collect()
}

fn send_loop(
    socket: UdpSocket,
    sample_rx: Receiver<Vec<i16>>,
    sample_rate: u32,
    frame_ms: u32,
    dropped_callbacks: Arc<AtomicU64>,
) -> Result<(), String> {
    let max_samples_per_packet = (MAX_PACKET_BYTES - HEADER_LEN) / 2;
    let target_samples = ((sample_rate as u64 * frame_ms as u64) / 1000)
        .max(1)
        .min(max_samples_per_packet as u64) as usize;
    let actual_frame_ms = target_samples as f64 * 1000.0 / sample_rate as f64;
    let mut samples = VecDeque::<i16>::new();
    let mut seq = 1_u64;
    let mut sent_packets = 0_u64;
    let mut sent_samples = 0_u64;
    let mut last_report = Instant::now();
    loop {
        match sample_rx.recv_timeout(Duration::from_millis(100)) {
            Ok(chunk) => samples.extend(chunk),
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
        }
        while samples.len() >= target_samples {
            let mut frame = Vec::with_capacity(target_samples);
            for _ in 0..target_samples {
                if let Some(sample) = samples.pop_front() {
                    frame.push(sample);
                }
            }
            let packet = encode_packet(seq, now_micros(), sample_rate, &frame);
            socket
                .send(&packet)
                .map_err(|error| format!("send udp packet failed: {error}"))?;
            seq = seq.saturating_add(1);
            sent_packets = sent_packets.saturating_add(1);
            sent_samples = sent_samples.saturating_add(frame.len() as u64);
        }
        if last_report.elapsed() >= Duration::from_secs(1) {
            println!(
                "tx packets={} audio_ms={} queued_ms={:.1} frame_ms={:.1} dropped_callbacks={}",
                sent_packets,
                sent_samples.saturating_mul(1000) / sample_rate as u64,
                samples.len() as f64 * 1000.0 / sample_rate as f64,
                actual_frame_ms,
                dropped_callbacks.load(Ordering::Relaxed)
            );
            last_report = Instant::now();
        }
    }
}

fn encode_packet(seq: u64, sent_micros: u64, sample_rate: u32, samples: &[i16]) -> Vec<u8> {
    let mut packet = Vec::with_capacity(HEADER_LEN + samples.len() * 2);
    packet.extend_from_slice(MAGIC);
    packet.extend_from_slice(&seq.to_be_bytes());
    packet.extend_from_slice(&sent_micros.to_be_bytes());
    packet.extend_from_slice(&sample_rate.to_be_bytes());
    packet.extend_from_slice(&1_u16.to_be_bytes());
    for sample in samples {
        packet.extend_from_slice(&sample.to_le_bytes());
    }
    packet
}

struct Packet {
    seq: u64,
    sent_micros: u64,
    sample_rate: u32,
    channels: u16,
    bytes: Vec<u8>,
}

fn decode_packet(packet: &[u8]) -> Option<Packet> {
    if packet.len() < HEADER_LEN || &packet[0..4] != MAGIC {
        return None;
    }
    let bytes = packet[HEADER_LEN..].to_vec();
    if bytes.len() % 2 != 0 {
        return None;
    }
    Some(Packet {
        seq: u64::from_be_bytes(packet[4..12].try_into().ok()?),
        sent_micros: u64::from_be_bytes(packet[12..20].try_into().ok()?),
        sample_rate: u32::from_be_bytes(packet[20..24].try_into().ok()?),
        channels: u16::from_be_bytes(packet[24..26].try_into().ok()?),
        bytes,
    })
}

fn recv_loop(socket: UdpSocket, player: AudioPlayer) -> Result<(), String> {
    let mut buf = [0_u8; 2048];
    let mut packets = 0_u64;
    let mut lost = 0_u64;
    let mut last_seq = None::<u64>;
    let mut last_report = Instant::now();
    loop {
        match socket.recv(&mut buf) {
            Ok(len) => {
                let Some(packet) = decode_packet(&buf[..len]) else {
                    continue;
                };
                if let Some(previous) = last_seq {
                    if packet.seq > previous.saturating_add(1) {
                        lost = lost.saturating_add(packet.seq - previous - 1);
                    }
                }
                last_seq = Some(packet.seq);
                packets = packets.saturating_add(1);
                player.push_frame(packet.sample_rate, packet.channels, &packet.bytes);
                if last_report.elapsed() >= Duration::from_secs(1) {
                    println!(
                        "rx packets={} lost={} last_seq={} buffered_ms={:.1} sender_clock_lag_ms={}",
                        packets,
                        lost,
                        packet.seq,
                        player.buffered_ms(),
                        now_micros().saturating_sub(packet.sent_micros) / 1000
                    );
                    last_report = Instant::now();
                }
            }
            Err(error)
                if error.kind() == std::io::ErrorKind::WouldBlock
                    || error.kind() == std::io::ErrorKind::TimedOut => {}
            Err(error) => return Err(format!("receive udp packet failed: {error}")),
        }
    }
}

struct AudioPlayer {
    buffer: Arc<Mutex<AudioPlaybackState>>,
    output_sample_rate: u32,
    output_channels: u16,
    _stream: cpal::Stream,
}

impl AudioPlayer {
    fn start() -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| "no default output device found".to_string())?;
        let config = device
            .default_output_config()
            .map_err(|error| format!("default output config failed: {error}"))?;
        let sample_format = config.sample_format();
        let config = config.config();
        let output_sample_rate = config.sample_rate.0;
        let output_channels = config.channels;
        let buffer = Arc::new(Mutex::new(AudioPlaybackState::new(
            output_sample_rate,
            output_channels,
        )));
        let stream = match sample_format {
            cpal::SampleFormat::F32 => build_f32_output_stream(&device, &config, buffer.clone()),
            cpal::SampleFormat::I16 => build_i16_output_stream(&device, &config, buffer.clone()),
            cpal::SampleFormat::U16 => build_u16_output_stream(&device, &config, buffer.clone()),
            other => Err(format!("unsupported output sample format: {other:?}")),
        }?;
        stream
            .play()
            .map_err(|error| format!("start output stream failed: {error}"))?;
        Ok(Self {
            buffer,
            output_sample_rate,
            output_channels,
            _stream: stream,
        })
    }

    fn push_frame(&self, sample_rate: u32, channels: u16, bytes: &[u8]) {
        let samples = pcm_s16le_to_f32(bytes);
        let converted = resample_and_map_channels(
            &samples,
            sample_rate,
            channels,
            self.output_sample_rate,
            self.output_channels,
        );
        if let Ok(mut buffer) = self.buffer.lock() {
            buffer.push_samples(converted);
        }
    }

    fn buffered_ms(&self) -> f64 {
        self.buffer
            .lock()
            .map(|buffer| buffer.buffered_ms())
            .unwrap_or_default()
    }
}

fn build_f32_output_stream(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    buffer: Arc<Mutex<AudioPlaybackState>>,
) -> Result<cpal::Stream, String> {
    device
        .build_output_stream(
            config,
            move |data: &mut [f32], _| fill_f32_output(data, &buffer),
            |error| eprintln!("output stream error: {error}"),
            None,
        )
        .map_err(|error| format!("build output stream failed: {error}"))
}

fn build_i16_output_stream(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    buffer: Arc<Mutex<AudioPlaybackState>>,
) -> Result<cpal::Stream, String> {
    device
        .build_output_stream(
            config,
            move |data: &mut [i16], _| fill_i16_output(data, &buffer),
            |error| eprintln!("output stream error: {error}"),
            None,
        )
        .map_err(|error| format!("build output stream failed: {error}"))
}

fn build_u16_output_stream(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    buffer: Arc<Mutex<AudioPlaybackState>>,
) -> Result<cpal::Stream, String> {
    device
        .build_output_stream(
            config,
            move |data: &mut [u16], _| fill_u16_output(data, &buffer),
            |error| eprintln!("output stream error: {error}"),
            None,
        )
        .map_err(|error| format!("build output stream failed: {error}"))
}

struct AudioPlaybackState {
    samples: VecDeque<f32>,
    started: bool,
    prebuffer_samples: usize,
    max_samples: usize,
    sample_rate: u32,
    channels: u16,
}

impl AudioPlaybackState {
    fn new(sample_rate: u32, channels: u16) -> Self {
        let samples_per_ms = sample_rate as usize * channels.max(1) as usize;
        Self {
            samples: VecDeque::new(),
            started: false,
            prebuffer_samples: (samples_per_ms * RX_PREBUFFER_MS / 1000).max(1),
            max_samples: (samples_per_ms * RX_MAX_BUFFER_MS / 1000).max(1),
            sample_rate,
            channels,
        }
    }

    fn push_samples(&mut self, samples: Vec<f32>) {
        self.samples.extend(samples);
        while self.samples.len() > self.max_samples {
            let _ = self.samples.pop_front();
            self.started = true;
        }
    }

    fn next_sample(&mut self) -> f32 {
        if !self.started {
            if self.samples.len() >= self.prebuffer_samples {
                self.started = true;
            } else {
                return 0.0;
            }
        }
        match self.samples.pop_front() {
            Some(sample) => sample,
            None => {
                self.started = false;
                0.0
            }
        }
    }

    fn buffered_ms(&self) -> f64 {
        let channels = self.channels.max(1) as f64;
        self.samples.len() as f64 * 1000.0 / self.sample_rate as f64 / channels
    }
}

fn fill_f32_output(data: &mut [f32], buffer: &Arc<Mutex<AudioPlaybackState>>) {
    if let Ok(mut buffer) = buffer.lock() {
        for sample in data {
            *sample = buffer.next_sample();
        }
    } else {
        data.fill(0.0);
    }
}

fn fill_i16_output(data: &mut [i16], buffer: &Arc<Mutex<AudioPlaybackState>>) {
    if let Ok(mut buffer) = buffer.lock() {
        for sample in data {
            let value = buffer.next_sample().clamp(-1.0, 1.0);
            *sample = (value * i16::MAX as f32).round() as i16;
        }
    } else {
        data.fill(0);
    }
}

fn fill_u16_output(data: &mut [u16], buffer: &Arc<Mutex<AudioPlaybackState>>) {
    if let Ok(mut buffer) = buffer.lock() {
        for sample in data {
            let value = buffer.next_sample().clamp(-1.0, 1.0);
            *sample =
                ((value * i16::MAX as f32).round() as i32 + 32768).clamp(0, u16::MAX as i32) as u16;
        }
    } else {
        data.fill(32768);
    }
}

fn pcm_s16le_to_f32(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(2)
        .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]) as f32 / 32768.0)
        .collect()
}

fn resample_and_map_channels(
    input: &[f32],
    input_rate: u32,
    input_channels: u16,
    output_rate: u32,
    output_channels: u16,
) -> Vec<f32> {
    let input_channels = input_channels.max(1) as usize;
    let output_channels = output_channels.max(1) as usize;
    let input_frames = input.len() / input_channels;
    if input_frames == 0 || input_rate == 0 || output_rate == 0 {
        return Vec::new();
    }
    let output_frames =
        ((input_frames as f64 * output_rate as f64) / input_rate as f64).ceil() as usize;
    let mut output = Vec::with_capacity(output_frames * output_channels);
    let rate_ratio = input_rate as f64 / output_rate as f64;
    for output_frame in 0..output_frames {
        let source_pos = output_frame as f64 * rate_ratio;
        let input_frame = (source_pos.floor() as usize).min(input_frames - 1);
        let next_frame = input_frame.saturating_add(1).min(input_frames - 1);
        let mix = (source_pos - input_frame as f64) as f32;
        for output_channel in 0..output_channels {
            let current = mapped_channel_sample(input, input_frame, input_channels, output_channel);
            let next = mapped_channel_sample(input, next_frame, input_channels, output_channel);
            output.push(current + (next - current) * mix);
        }
    }
    output
}

fn mapped_channel_sample(
    input: &[f32],
    frame: usize,
    input_channels: usize,
    output_channel: usize,
) -> f32 {
    if input_channels == 1 {
        return input[frame * input_channels];
    }
    if output_channel < input_channels {
        return input[frame * input_channels + output_channel];
    }
    let start = frame * input_channels;
    let sum: f32 = input[start..start + input_channels].iter().copied().sum();
    sum / input_channels as f32
}

fn now_micros() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_micros().min(u64::MAX as u128) as u64)
        .unwrap_or_default()
}
