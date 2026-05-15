use base64::Engine;
use image::codecs::jpeg::JpegEncoder;
#[cfg(target_os = "windows")]
use image::DynamicImage;
#[cfg(target_os = "windows")]
use nokhwa::{
    pixel_format::RgbFormat,
    query,
    utils::{ApiBackend, CameraIndex, RequestedFormat, RequestedFormatType},
    Camera,
};
#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::fs;
#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::path::PathBuf;
#[cfg(any(target_os = "linux", target_os = "macos"))]
use std::process::Command;
#[cfg(target_os = "windows")]
use std::sync::{mpsc, Mutex, OnceLock};
#[cfg(target_os = "windows")]
use std::time::Duration;

pub fn handle(payload: &str) -> String {
    let request = CameraRequest::parse(payload);
    match request.action.as_str() {
        "devices" => list_devices(),
        "stop" => stop_camera(),
        "capture" | "" => capture_default_camera(&request),
        _ => format!(
            "camera_error\nmessage=unsupported action {}",
            request.action
        ),
    }
}

#[derive(Default)]
struct CameraRequest {
    action: String,
    device: usize,
    quality: String,
}

impl CameraRequest {
    fn parse(payload: &str) -> Self {
        let mut request = Self {
            action: "capture".to_string(),
            device: 0,
            quality: "medium".to_string(),
        };
        for line in payload.lines() {
            if let Some(rest) = line.strip_prefix("action=") {
                request.action = rest.trim().to_ascii_lowercase();
            } else if let Some(rest) = line.strip_prefix("device=") {
                request.device = rest.trim().parse().unwrap_or_default();
            } else if let Some(rest) = line.strip_prefix("quality=") {
                request.quality = match rest.trim().to_ascii_lowercase().as_str() {
                    "low" => "low".to_string(),
                    "high" => "high".to_string(),
                    _ => "medium".to_string(),
                };
            }
        }
        request
    }
}

fn stop_camera() -> String {
    #[cfg(target_os = "windows")]
    {
        return windows_stop_camera();
    }
    #[cfg(not(target_os = "windows"))]
    "camera_stopped".to_string()
}

fn list_devices() -> String {
    #[cfg(target_os = "windows")]
    {
        return windows_list_devices();
    }
    #[cfg(target_os = "linux")]
    {
        return "camera_devices\ndevice\t0\tDefault camera\t/dev/video0".to_string();
    }
    #[cfg(target_os = "macos")]
    {
        return "camera_devices\ndevice\t0\tDefault camera\timagesnap default device".to_string();
    }
    #[allow(unreachable_code)]
    "camera_error\nmessage=camera device listing is not implemented for this platform".to_string()
}

fn capture_default_camera(request: &CameraRequest) -> String {
    #[cfg(target_os = "linux")]
    {
        return linux_capture(request);
    }
    #[cfg(target_os = "macos")]
    {
        return macos_capture(request);
    }
    #[cfg(target_os = "windows")]
    {
        return windows_capture(request);
    }
    #[allow(unreachable_code)]
    "camera_error\nmessage=camera capture is not implemented for this platform".to_string()
}

#[cfg(target_os = "linux")]
fn linux_capture(request: &CameraRequest) -> String {
    let path = temp_path("rdl-camera", "jpg");
    let path_text = path.to_string_lossy().to_string();
    let result = run_capture(
        "ffmpeg",
        &[
            "-y",
            "-f",
            "video4linux2",
            "-frames:v",
            "1",
            "-i",
            "/dev/video0",
            &path_text,
        ],
    )
    .or_else(|_| run_capture("fswebcam", &["--no-banner", "-r", "1280x720", &path_text]))
    .or_else(|_| run_capture("streamer", &["-f", "jpeg", "-o", &path_text]));
    finish_capture(path, result, request)
}

#[cfg(target_os = "macos")]
fn macos_capture(request: &CameraRequest) -> String {
    let path = temp_path("rdl-camera", "jpg");
    let path_text = path.to_string_lossy().to_string();
    finish_capture(path, run_capture("imagesnap", &[&path_text]), request)
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn finish_capture(path: PathBuf, result: Result<(), String>, request: &CameraRequest) -> String {
    if let Err(error) = result {
        let _ = fs::remove_file(&path);
        return format!("camera_error\nmessage={error}");
    }
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) => return format!("camera_error\nmessage=read camera frame failed: {error}"),
    };
    let _ = fs::remove_file(&path);
    let image = match image::load_from_memory(&bytes) {
        Ok(image) => image,
        Err(error) => return format!("camera_error\nmessage=load camera frame failed: {error}"),
    };
    let (bytes, width, height) = match encode_camera_image(image, &request.quality) {
        Ok(encoded) => encoded,
        Err(error) => return format!("camera_error\nmessage=encode camera frame failed: {error}"),
    };
    format!(
        "camera_frame\ndevice={}\nformat=jpeg\nwidth={}\nheight={}\nbytes={}\nimage_base64={}",
        request.device,
        width,
        height,
        bytes.len(),
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}

#[cfg(target_os = "windows")]
fn windows_list_devices() -> String {
    let devices = match query(ApiBackend::MediaFoundation) {
        Ok(devices) => devices,
        Err(error) => return format!("camera_error\nmessage=list camera devices failed: {error}"),
    };
    let mut text = String::from("camera_devices");
    for (index, device) in devices.iter().enumerate() {
        text.push_str(&format!(
            "\ndevice\t{}\t{}\t{}",
            index,
            sanitize_field(&device.human_name()),
            sanitize_field(device.description())
        ));
    }
    text
}

#[cfg(target_os = "windows")]
fn windows_capture(request: &CameraRequest) -> String {
    let (reply_tx, reply_rx) = mpsc::channel();
    let request_message = WindowsCameraRequest::Capture {
        device: request.device,
        quality: request.quality.clone(),
        reply: reply_tx,
    };
    match send_windows_camera_request(request_message) {
        Ok(()) => reply_rx.recv().unwrap_or_else(|error| {
            format!("camera_error\nmessage=camera worker stopped: {error}")
        }),
        Err(error) => format!("camera_error\nmessage={error}"),
    }
}

#[cfg(target_os = "windows")]
fn windows_stop_camera() -> String {
    let (reply_tx, reply_rx) = mpsc::channel();
    match send_windows_camera_request(WindowsCameraRequest::Stop { reply: reply_tx }) {
        Ok(()) => reply_rx.recv().unwrap_or_else(|error| {
            format!("camera_error\nmessage=camera worker stopped: {error}")
        }),
        Err(error) => format!("camera_error\nmessage={error}"),
    }
}

#[cfg(target_os = "windows")]
struct WindowsCameraSession {
    device: usize,
    camera: Camera,
}

#[cfg(target_os = "windows")]
enum WindowsCameraRequest {
    Capture {
        device: usize,
        quality: String,
        reply: mpsc::Sender<String>,
    },
    Stop {
        reply: mpsc::Sender<String>,
    },
}

#[cfg(target_os = "windows")]
struct WindowsCameraWorker {
    tx: mpsc::Sender<WindowsCameraRequest>,
}

#[cfg(target_os = "windows")]
fn send_windows_camera_request(request: WindowsCameraRequest) -> Result<(), String> {
    let tx = windows_camera_worker()?;
    if tx.send(request).is_ok() {
        return Ok(());
    }
    reset_windows_camera_worker();
    Err("camera worker stopped before accepting request".to_string())
}

#[cfg(target_os = "windows")]
fn windows_camera_index(device_index: usize) -> Result<CameraIndex, String> {
    let devices = query(ApiBackend::MediaFoundation)
        .map_err(|error| format!("list camera devices failed: {error}"))?;
    let Some(device) = devices.get(device_index).or_else(|| devices.first()) else {
        return Err("no Windows camera device found".to_string());
    };
    Ok(device.index().clone())
}

#[cfg(target_os = "windows")]
fn windows_camera_worker() -> Result<mpsc::Sender<WindowsCameraRequest>, String> {
    let mut guard = windows_camera_worker_slot()
        .lock()
        .map_err(|_| "camera worker lock is poisoned".to_string())?;
    if guard.is_none() {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || windows_camera_worker_loop(rx));
        *guard = Some(WindowsCameraWorker { tx });
    }
    Ok(guard.as_ref().expect("worker initialized").tx.clone())
}

#[cfg(target_os = "windows")]
fn reset_windows_camera_worker() {
    if let Ok(mut guard) = windows_camera_worker_slot().lock() {
        *guard = None;
    }
}

#[cfg(target_os = "windows")]
fn windows_camera_worker_slot() -> &'static Mutex<Option<WindowsCameraWorker>> {
    static WORKER: OnceLock<Mutex<Option<WindowsCameraWorker>>> = OnceLock::new();
    WORKER.get_or_init(|| Mutex::new(None))
}

#[cfg(target_os = "windows")]
fn windows_camera_worker_loop(rx: mpsc::Receiver<WindowsCameraRequest>) {
    let mut session: Option<WindowsCameraSession> = None;
    for request in rx {
        match request {
            WindowsCameraRequest::Capture {
                device,
                quality,
                reply,
            } => {
                let result = windows_capture_on_worker(&mut session, device, &quality);
                let _ = reply.send(result);
            }
            WindowsCameraRequest::Stop { reply } => {
                session = None;
                let _ = reply.send("camera_stopped".to_string());
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn windows_capture_on_worker(
    session: &mut Option<WindowsCameraSession>,
    device: usize,
    quality: &str,
) -> String {
    if session
        .as_ref()
        .is_none_or(|session| session.device != device)
    {
        match open_windows_camera_session(device) {
            Ok(new_session) => *session = Some(new_session),
            Err(error) => return format!("camera_error\nmessage={error}"),
        }
    }
    let Some(session) = session.as_mut() else {
        return "camera_error\nmessage=camera session is not available".to_string();
    };
    let frame = match session.camera.frame() {
        Ok(frame) => frame,
        Err(error) => return format!("camera_error\nmessage=capture camera frame failed: {error}"),
    };
    let decoded = match frame.decode_image::<RgbFormat>() {
        Ok(decoded) => decoded,
        Err(error) => return format!("camera_error\nmessage=decode camera frame failed: {error}"),
    };
    let image = DynamicImage::ImageRgb8(decoded);
    let (bytes, width, height) = match encode_camera_image(image, quality) {
        Ok(encoded) => encoded,
        Err(error) => return format!("camera_error\nmessage=encode camera frame failed: {error}"),
    };
    format!(
        "camera_frame\ndevice={}\nformat=jpeg\nwidth={}\nheight={}\nbytes={}\nimage_base64={}",
        device,
        width,
        height,
        bytes.len(),
        base64::engine::general_purpose::STANDARD.encode(bytes)
    )
}

#[cfg(target_os = "windows")]
fn open_windows_camera_session(device: usize) -> Result<WindowsCameraSession, String> {
    let camera_index = windows_camera_index(device)?;
    let requested =
        RequestedFormat::new::<RgbFormat>(RequestedFormatType::AbsoluteHighestFrameRate);
    let mut camera = Camera::new(camera_index, requested)
        .map_err(|error| format!("open camera failed: {error}"))?;
    camera
        .open_stream()
        .map_err(|error| format!("open camera stream failed: {error}"))?;

    std::thread::sleep(Duration::from_millis(220));
    for _ in 0..5 {
        let _ = camera.frame();
        std::thread::sleep(Duration::from_millis(30));
    }

    Ok(WindowsCameraSession { device, camera })
}

fn encode_camera_image(
    image: image::DynamicImage,
    quality: &str,
) -> Result<(Vec<u8>, u32, u32), image::ImageError> {
    let (max_width, jpeg_quality) = quality_profile(quality);
    let image = if image.width() > max_width {
        image.resize(max_width, u32::MAX, image::imageops::FilterType::Triangle)
    } else {
        image
    };
    let width = image.width();
    let height = image.height();
    let mut bytes = Vec::new();
    let mut encoder = JpegEncoder::new_with_quality(&mut bytes, jpeg_quality);
    encoder.encode_image(&image)?;
    Ok((bytes, width, height))
}

fn quality_profile(value: &str) -> (u32, u8) {
    match value {
        "low" => (640, 42),
        "high" => (1920, 88),
        _ => (1280, 72),
    }
}

#[cfg(target_os = "windows")]
fn sanitize_field(value: &str) -> String {
    value.replace(['\r', '\n', '\t'], " ").trim().to_string()
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn run_capture(program: &str, args: &[&str]) -> Result<(), String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|error| format!("{program} failed: {error}"))?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "{program} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

#[cfg(any(target_os = "linux", target_os = "macos"))]
fn temp_path(prefix: &str, ext: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "{prefix}-{}-{}.{}",
        std::process::id(),
        rdl_protocol::now_epoch_ms(),
        ext
    ))
}
