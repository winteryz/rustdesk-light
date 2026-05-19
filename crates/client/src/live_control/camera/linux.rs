use super::{encode_camera_image, CameraCapture, CameraRequest, CameraVideoFrame};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(target_os = "linux")]
pub(super) fn capture_frame(request: &CameraRequest) -> Result<CameraVideoFrame, String> {
    let mut capture = CameraCapture::new(request.device, &request.quality)?;
    capture.capture_frame()
}

#[cfg(target_os = "linux")]
pub(super) fn capture_stream_frame(
    capture: &mut CameraCapture,
) -> Result<CameraVideoFrame, String> {
    let mut last_error = String::new();
    for offset in 0..capture.backends.len() {
        let index = (capture.active_backend + offset) % capture.backends.len();
        match capture.backends[index]
            .capture(&capture.device_path)
            .and_then(|bytes| encode_camera_bytes(bytes, &capture.quality))
        {
            Ok(frame) => {
                capture.active_backend = index;
                return Ok(frame);
            }
            Err(error) => {
                last_error = error;
            }
        }
    }
    Err(if last_error.trim().is_empty() {
        "Linux camera capture requires ffmpeg, fswebcam, or streamer".to_string()
    } else {
        last_error
    })
}

#[cfg(target_os = "linux")]
#[derive(Clone, Copy)]
pub(super) enum CameraBackend {
    FfmpegStdout,
    FfmpegFile,
    FswebcamFile,
    StreamerFile,
}

#[cfg(target_os = "linux")]
impl CameraBackend {
    fn capture(self, device_path: &str) -> Result<Vec<u8>, String> {
        match self {
            Self::FfmpegStdout => run_capture_stdout(
                "ffmpeg",
                &[
                    "-hide_banner",
                    "-loglevel",
                    "error",
                    "-f",
                    "video4linux2",
                    "-i",
                    device_path,
                    "-frames:v",
                    "1",
                    "-f",
                    "image2pipe",
                    "-vcodec",
                    "mjpeg",
                    "-",
                ],
            ),
            Self::FfmpegFile => {
                let path = temp_path("rdl-camera", "jpg");
                let path_text = path.to_string_lossy().to_string();
                run_capture_file(
                    "ffmpeg",
                    &[
                        "-y",
                        "-hide_banner",
                        "-loglevel",
                        "error",
                        "-f",
                        "video4linux2",
                        "-i",
                        device_path,
                        "-frames:v",
                        "1",
                        &path_text,
                    ],
                    &path,
                )
            }
            Self::FswebcamFile => {
                let path = temp_path("rdl-camera", "jpg");
                let path_text = path.to_string_lossy().to_string();
                run_capture_file(
                    "fswebcam",
                    &[
                        "--no-banner",
                        "--device",
                        device_path,
                        "-r",
                        "1280x720",
                        &path_text,
                    ],
                    &path,
                )
            }
            Self::StreamerFile => {
                let path = temp_path("rdl-camera", "jpg");
                let path_text = path.to_string_lossy().to_string();
                run_capture_file(
                    "streamer",
                    &["-c", device_path, "-f", "jpeg", "-o", &path_text],
                    &path,
                )
            }
        }
    }
}

#[cfg(target_os = "linux")]
fn encode_camera_bytes(bytes: Vec<u8>, quality: &str) -> Result<CameraVideoFrame, String> {
    let image = image::load_from_memory(&bytes)
        .map_err(|error| format!("load camera frame failed: {error}"))?;
    let (bytes, width, height) = encode_camera_image(image, quality)
        .map_err(|error| format!("encode camera frame failed: {error}"))?;
    Ok(CameraVideoFrame {
        width,
        height,
        format: "jpeg".to_string(),
        bytes,
    })
}

#[cfg(target_os = "linux")]
pub(super) fn list_devices() -> String {
    let mut output = String::from("camera_devices");
    let devices = linux_camera_devices();
    if devices.is_empty() {
        output.push_str("\ndevice\t0\tDefault camera\t/dev/video0");
        return output;
    }
    for (index, path) in devices {
        output.push_str(&format!(
            "\ndevice\t{}\t{}\t{}",
            index,
            sanitize_linux_field(&format!("Camera {index}")),
            sanitize_linux_field(&path)
        ));
    }
    output
}

#[cfg(target_os = "linux")]
fn linux_camera_devices() -> Vec<(usize, String)> {
    let mut devices = fs::read_dir("/dev")
        .ok()
        .into_iter()
        .flat_map(|entries| entries.filter_map(Result::ok))
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().to_string();
            let index = name.strip_prefix("video")?.parse::<usize>().ok()?;
            Some((index, format!("/dev/{name}")))
        })
        .collect::<Vec<_>>();
    devices.sort_by_key(|(index, _)| *index);
    devices
}

#[cfg(target_os = "linux")]
pub(super) fn device_path(device: usize) -> String {
    let path = format!("/dev/video{device}");
    if Path::new(&path).exists() {
        path
    } else {
        "/dev/video0".to_string()
    }
}

#[cfg(target_os = "linux")]
pub(super) fn camera_backends() -> Result<Vec<CameraBackend>, String> {
    let mut backends = Vec::new();
    if command_in_path("ffmpeg") {
        backends.push(CameraBackend::FfmpegStdout);
        backends.push(CameraBackend::FfmpegFile);
    }
    if command_in_path("fswebcam") {
        backends.push(CameraBackend::FswebcamFile);
    }
    if command_in_path("streamer") {
        backends.push(CameraBackend::StreamerFile);
    }
    if backends.is_empty() {
        Err("Linux camera capture requires ffmpeg, fswebcam, or streamer".to_string())
    } else {
        Ok(backends)
    }
}

#[cfg(target_os = "linux")]
fn command_in_path(program: &str) -> bool {
    let Some(paths) = env::var_os("PATH") else {
        return false;
    };
    env::split_paths(&paths).any(|dir| dir.join(program).is_file())
}

#[cfg(target_os = "linux")]
fn run_capture_stdout(program: &str, args: &[&str]) -> Result<Vec<u8>, String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|error| format!("{program} failed: {error}"))?;
    if !output.status.success() {
        return Err(format!(
            "{program} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    if output.stdout.is_empty() {
        return Err(format!("{program} produced an empty camera frame"));
    }
    Ok(output.stdout)
}

#[cfg(target_os = "linux")]
fn run_capture_file(program: &str, args: &[&str], path: &Path) -> Result<Vec<u8>, String> {
    let output = Command::new(program)
        .args(args)
        .output()
        .map_err(|error| format!("{program} failed: {error}"))?;
    if !output.status.success() {
        let _ = fs::remove_file(path);
        return Err(format!(
            "{program} failed: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    let bytes = fs::read(path).map_err(|error| format!("read camera frame failed: {error}"))?;
    let _ = fs::remove_file(path);
    if bytes.is_empty() {
        return Err(format!("{program} produced an empty camera frame"));
    }
    Ok(bytes)
}

#[cfg(target_os = "linux")]
fn sanitize_linux_field(value: &str) -> String {
    value.replace(['\r', '\n', '\t'], " ").trim().to_string()
}

#[cfg(target_os = "linux")]
fn temp_path(prefix: &str, ext: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "{prefix}-{}-{}.{}",
        std::process::id(),
        rdl_protocol::now_epoch_ms(),
        ext
    ))
}
