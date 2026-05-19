use super::{encode_camera_image, sanitize_field, CameraRequest, CameraVideoFrame};
use image::DynamicImage;
use nokhwa::{
    pixel_format::RgbFormat,
    query,
    utils::{ApiBackend, CameraIndex, RequestedFormat, RequestedFormatType},
    Camera,
};
use std::sync::{mpsc, Mutex, OnceLock};
use std::time::Duration;

#[cfg(target_os = "windows")]
pub(super) fn list_devices() -> String {
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
pub(super) fn capture_frame(request: &CameraRequest) -> Result<CameraVideoFrame, String> {
    let (reply_tx, reply_rx) = mpsc::channel();
    let request_message = WindowsCameraRequest::Capture {
        device: request.device,
        quality: request.quality.clone(),
        reply: reply_tx,
    };
    match send_windows_camera_request(request_message) {
        Ok(()) => reply_rx
            .recv()
            .unwrap_or_else(|error| Err(format!("camera worker stopped: {error}"))),
        Err(error) => Err(error),
    }
}

#[cfg(target_os = "windows")]
pub(super) fn stop_camera() -> String {
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
        reply: mpsc::Sender<Result<CameraVideoFrame, String>>,
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
                let result = windows_capture_on_worker_frame(&mut session, device, &quality);
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
fn windows_capture_on_worker_frame(
    session: &mut Option<WindowsCameraSession>,
    device: usize,
    quality: &str,
) -> Result<CameraVideoFrame, String> {
    if session
        .as_ref()
        .is_none_or(|session| session.device != device)
    {
        match open_windows_camera_session(device) {
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
