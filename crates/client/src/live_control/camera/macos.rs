use super::{encode_camera_image, sanitize_field, CameraRequest, CameraVideoFrame};
use image::DynamicImage;
use nokhwa::{
    nokhwa_check, nokhwa_initialize,
    pixel_format::RgbFormat,
    query,
    utils::{ApiBackend, CameraIndex, RequestedFormat, RequestedFormatType},
    Camera,
};
use std::sync::{mpsc, Mutex, OnceLock};
use std::time::Duration;

#[cfg(target_os = "macos")]
pub(super) fn capture_frame(request: &CameraRequest) -> Result<CameraVideoFrame, String> {
    let (reply_tx, reply_rx) = mpsc::channel();
    let request_message = MacosCameraRequest::Capture {
        device: request.device,
        quality: request.quality.clone(),
        reply: reply_tx,
    };
    match send_macos_camera_request(request_message) {
        Ok(()) => reply_rx
            .recv()
            .unwrap_or_else(|error| Err(format!("camera worker stopped: {error}"))),
        Err(error) => Err(error),
    }
}

#[cfg(target_os = "macos")]
pub(super) fn list_devices() -> String {
    let devices = match query(ApiBackend::AVFoundation) {
        Ok(devices) => devices,
        Err(error) => return format!("camera_error\nmessage=list camera devices failed: {error}"),
    };
    let mut text = String::from("camera_devices");
    for (index, device) in devices.iter().enumerate() {
        text.push_str(&format!(
            "\ndevice\t{}\t{}\t{}",
            index,
            sanitize_field(&device.human_name()),
            macos_device_description(device.description())
        ));
    }
    text
}

#[cfg(target_os = "macos")]
fn macos_device_description(description: &str) -> String {
    let description = sanitize_field(description);
    description
        .split_once(':')
        .map(|(_, rest)| rest.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or(description)
}

#[cfg(target_os = "macos")]
fn macos_ensure_camera_permission() -> Result<(), String> {
    if nokhwa_check() {
        return Ok(());
    }

    let (tx, rx) = mpsc::channel();
    nokhwa_initialize(move |granted| {
        let _ = tx.send(granted);
    });
    match rx.recv_timeout(Duration::from_secs(30)) {
        Ok(true) => Ok(()),
        Ok(false) => Err("camera permission denied; grant Camera permission to this app in macOS System Settings".to_string()),
        Err(_) => Err("camera permission request timed out; grant Camera permission to this app in macOS System Settings".to_string()),
    }
}

#[cfg(target_os = "macos")]
pub(super) fn stop_camera() -> String {
    let (reply_tx, reply_rx) = mpsc::channel();
    match send_macos_camera_request(MacosCameraRequest::Stop { reply: reply_tx }) {
        Ok(()) => reply_rx.recv().unwrap_or_else(|error| {
            format!("camera_error\nmessage=camera worker stopped: {error}")
        }),
        Err(error) => format!("camera_error\nmessage={error}"),
    }
}

#[cfg(target_os = "macos")]
struct MacosCameraSession {
    device: usize,
    camera: Camera,
}

#[cfg(target_os = "macos")]
enum MacosCameraRequest {
    Capture {
        device: usize,
        quality: String,
        reply: mpsc::Sender<Result<CameraVideoFrame, String>>,
    },
    Stop {
        reply: mpsc::Sender<String>,
    },
}

#[cfg(target_os = "macos")]
struct MacosCameraWorker {
    tx: mpsc::Sender<MacosCameraRequest>,
}

#[cfg(target_os = "macos")]
fn send_macos_camera_request(request: MacosCameraRequest) -> Result<(), String> {
    let tx = macos_camera_worker()?;
    if tx.send(request).is_ok() {
        return Ok(());
    }
    reset_macos_camera_worker();
    Err("camera worker stopped before accepting request".to_string())
}

#[cfg(target_os = "macos")]
fn macos_camera_worker() -> Result<mpsc::Sender<MacosCameraRequest>, String> {
    let mut guard = macos_camera_worker_slot()
        .lock()
        .map_err(|_| "camera worker lock is poisoned".to_string())?;
    if guard.is_none() {
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || macos_camera_worker_loop(rx));
        *guard = Some(MacosCameraWorker { tx });
    }
    guard
        .as_ref()
        .map(|worker| worker.tx.clone())
        .ok_or_else(|| "camera worker is not available".to_string())
}

#[cfg(target_os = "macos")]
fn reset_macos_camera_worker() {
    if let Ok(mut guard) = macos_camera_worker_slot().lock() {
        *guard = None;
    }
}

#[cfg(target_os = "macos")]
fn macos_camera_worker_slot() -> &'static Mutex<Option<MacosCameraWorker>> {
    static WORKER: OnceLock<Mutex<Option<MacosCameraWorker>>> = OnceLock::new();
    WORKER.get_or_init(|| Mutex::new(None))
}

#[cfg(target_os = "macos")]
fn macos_camera_worker_loop(rx: mpsc::Receiver<MacosCameraRequest>) {
    let mut session: Option<MacosCameraSession> = None;
    while let Ok(request) = rx.recv() {
        match request {
            MacosCameraRequest::Capture {
                device,
                quality,
                reply,
            } => {
                let _ = reply.send(macos_capture_on_worker_frame(
                    &mut session,
                    device,
                    &quality,
                ));
            }
            MacosCameraRequest::Stop { reply } => {
                session = None;
                let _ = reply.send("camera_stopped".to_string());
            }
        }
    }
}

#[cfg(target_os = "macos")]
fn macos_capture_on_worker_frame(
    session: &mut Option<MacosCameraSession>,
    device: usize,
    quality: &str,
) -> Result<CameraVideoFrame, String> {
    if session
        .as_ref()
        .is_none_or(|session| session.device != device)
    {
        match open_macos_camera_session(device) {
            Ok(new_session) => *session = Some(new_session),
            Err(error) => return Err(error),
        }
    }
    let Some(session) = session.as_mut() else {
        return Err("camera session is not available".to_string());
    };
    let frame = match session.camera.frame() {
        Ok(frame) => frame,
        Err(error) => return Err(format!("capture camera frame failed: {error}")),
    };
    let decoded = match frame.decode_image::<RgbFormat>() {
        Ok(decoded) => decoded,
        Err(error) => return Err(format!("decode camera frame failed: {error}")),
    };
    let image = DynamicImage::ImageRgb8(decoded);
    let (bytes, width, height) = match encode_camera_image(image, quality) {
        Ok(encoded) => encoded,
        Err(error) => return Err(format!("encode camera frame failed: {error}")),
    };
    Ok(CameraVideoFrame {
        width,
        height,
        format: "jpeg".to_string(),
        bytes,
    })
}

#[cfg(target_os = "macos")]
fn open_macos_camera_session(device: usize) -> Result<MacosCameraSession, String> {
    macos_ensure_camera_permission()?;
    let camera_index = macos_camera_index(device)?;
    let requested =
        RequestedFormat::new::<RgbFormat>(RequestedFormatType::AbsoluteHighestFrameRate);
    let mut camera = Camera::with_backend(camera_index, requested, ApiBackend::AVFoundation)
        .map_err(|error| format!("open camera failed: {error}"))?;
    camera
        .open_stream()
        .map_err(|error| format!("open camera stream failed: {error}"))?;

    std::thread::sleep(Duration::from_millis(220));
    for _ in 0..5 {
        let _ = camera.frame();
        std::thread::sleep(Duration::from_millis(30));
    }

    Ok(MacosCameraSession { device, camera })
}

#[cfg(target_os = "macos")]
fn macos_camera_index(device_index: usize) -> Result<CameraIndex, String> {
    let devices = query(ApiBackend::AVFoundation)
        .map_err(|error| format!("list camera devices failed: {error}"))?;
    let Some(device) = devices.get(device_index).or_else(|| devices.first()) else {
        return Err("no macOS camera device found".to_string());
    };
    Ok(device.index().clone())
}
