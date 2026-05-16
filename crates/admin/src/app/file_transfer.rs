use super::event::AdminInput;
use rdl_protocol::{FileTransferAction, FileTransferDirection, Message};
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    mpsc::{SyncSender, TrySendError},
    Arc,
};
use std::thread;
use std::time::Duration;

const FILE_TRANSFER_CHUNK_SIZE: usize = 512 * 1024;

pub(super) fn should_log_admin_file_transfer_event(
    action: FileTransferAction,
    message: &str,
) -> bool {
    matches!(
        action,
        FileTransferAction::Start
            | FileTransferAction::Cancel
            | FileTransferAction::Complete
            | FileTransferAction::Error
    ) || !message.trim().is_empty()
}

pub(super) fn sanitize_log_value(value: &str) -> String {
    let mut value = value
        .chars()
        .map(|ch| if ch.is_control() { ' ' } else { ch })
        .collect::<String>();
    const MAX_LOG_VALUE_LEN: usize = 180;
    if value.len() > MAX_LOG_VALUE_LEN {
        value.truncate(MAX_LOG_VALUE_LEN);
        value.push_str("...");
    }
    value
}

pub(super) fn run_file_upload_transfer(
    input_tx: &SyncSender<AdminInput>,
    client_id: &str,
    transfer_id: u64,
    local_path: &str,
    remote_path: &str,
    cancel_flag: Arc<AtomicBool>,
) -> io::Result<()> {
    let source = PathBuf::from(local_path);
    let metadata = fs::metadata(&source)?;
    let mut dirs = Vec::new();
    let mut files = Vec::new();
    collect_upload_entries(&source, Path::new(""), &metadata, &mut dirs, &mut files)?;
    let total_bytes = files.iter().map(|file| file.size).sum::<u64>();
    let mut transferred_bytes = 0u64;

    send_file_transfer_input_cancelable(
        input_tx,
        file_transfer_message(
            client_id.to_string(),
            transfer_id,
            FileTransferDirection::Upload,
            FileTransferAction::Start,
            remote_path.to_string(),
            String::new(),
            total_bytes,
            0,
            0,
            0,
            Vec::new(),
            "upload started".to_string(),
        ),
        &cancel_flag,
    )?;

    for dir in dirs {
        if cancel_flag.load(Ordering::Relaxed) {
            return send_upload_cancel(input_tx, client_id, transfer_id, remote_path);
        }
        send_file_transfer_input_cancelable(
            input_tx,
            file_transfer_message(
                client_id.to_string(),
                transfer_id,
                FileTransferDirection::Upload,
                FileTransferAction::Directory,
                remote_path.to_string(),
                protocol_relative_path(&dir),
                total_bytes,
                transferred_bytes,
                0,
                0,
                Vec::new(),
                String::new(),
            ),
            &cancel_flag,
        )?;
    }

    let mut buffer = vec![0u8; FILE_TRANSFER_CHUNK_SIZE];
    for file in files {
        if cancel_flag.load(Ordering::Relaxed) {
            return send_upload_cancel(input_tx, client_id, transfer_id, remote_path);
        }
        let mut input = File::open(&file.path)?;
        let mut offset = 0u64;
        let relative_path = protocol_relative_path(&file.relative);
        loop {
            if cancel_flag.load(Ordering::Relaxed) {
                return send_upload_cancel(input_tx, client_id, transfer_id, remote_path);
            }
            let count = input.read(&mut buffer)?;
            if count == 0 {
                break;
            }
            transferred_bytes = transferred_bytes.saturating_add(count as u64);
            send_file_transfer_input_cancelable(
                input_tx,
                file_transfer_message(
                    client_id.to_string(),
                    transfer_id,
                    FileTransferDirection::Upload,
                    FileTransferAction::Chunk,
                    remote_path.to_string(),
                    relative_path.clone(),
                    total_bytes,
                    transferred_bytes,
                    file.size,
                    offset,
                    buffer[..count].to_vec(),
                    String::new(),
                ),
                &cancel_flag,
            )?;
            offset = offset.saturating_add(count as u64);
        }
    }

    send_file_transfer_input_cancelable(
        input_tx,
        file_transfer_message(
            client_id.to_string(),
            transfer_id,
            FileTransferDirection::Upload,
            FileTransferAction::Finish,
            remote_path.to_string(),
            String::new(),
            total_bytes,
            transferred_bytes,
            0,
            0,
            Vec::new(),
            "upload finished".to_string(),
        ),
        &cancel_flag,
    )
}

pub(super) fn send_upload_cancel(
    input_tx: &SyncSender<AdminInput>,
    client_id: &str,
    transfer_id: u64,
    remote_path: &str,
) -> io::Result<()> {
    send_file_transfer_input(
        input_tx,
        file_transfer_message(
            client_id.to_string(),
            transfer_id,
            FileTransferDirection::Upload,
            FileTransferAction::Cancel,
            remote_path.to_string(),
            String::new(),
            0,
            0,
            0,
            0,
            Vec::new(),
            "upload cancelled".to_string(),
        ),
    )
}

#[derive(Clone)]
struct UploadFileEntry {
    path: PathBuf,
    relative: PathBuf,
    size: u64,
}

fn collect_upload_entries(
    path: &Path,
    relative: &Path,
    metadata: &fs::Metadata,
    dirs: &mut Vec<PathBuf>,
    files: &mut Vec<UploadFileEntry>,
) -> io::Result<()> {
    if metadata.is_dir() {
        dirs.push(relative.to_path_buf());
        let mut children = fs::read_dir(path)?.flatten().collect::<Vec<_>>();
        children.sort_by_key(|entry| entry.file_name());
        for child in children {
            let child_metadata = child.metadata()?;
            let child_relative = relative.join(child.file_name());
            collect_upload_entries(&child.path(), &child_relative, &child_metadata, dirs, files)?;
        }
    } else {
        files.push(UploadFileEntry {
            path: path.to_path_buf(),
            relative: relative.to_path_buf(),
            size: metadata.len(),
        });
    }
    Ok(())
}

fn protocol_relative_path(path: &Path) -> String {
    path.components()
        .filter_map(|component| match component {
            std::path::Component::Normal(part) => Some(part.to_string_lossy().to_string()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/")
}

pub(super) fn send_file_transfer_input(
    input_tx: &SyncSender<AdminInput>,
    message: Message,
) -> io::Result<()> {
    input_tx
        .send(AdminInput::FileTransfer(message))
        .map_err(|error| io::Error::new(io::ErrorKind::BrokenPipe, error.to_string()))
}

fn send_file_transfer_input_cancelable(
    input_tx: &SyncSender<AdminInput>,
    message: Message,
    cancel_flag: &AtomicBool,
) -> io::Result<()> {
    let mut input = AdminInput::FileTransfer(message);
    loop {
        if cancel_flag.load(Ordering::Relaxed) {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "file upload cancelled",
            ));
        }
        match input_tx.try_send(input) {
            Ok(()) => return Ok(()),
            Err(TrySendError::Full(returned)) => {
                input = returned;
                thread::sleep(Duration::from_millis(5));
            }
            Err(TrySendError::Disconnected(returned)) => {
                drop(returned);
                return Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "admin input queue disconnected",
                ));
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn file_transfer_message(
    target_id: String,
    transfer_id: u64,
    direction: FileTransferDirection,
    action: FileTransferAction,
    path: String,
    relative_path: String,
    total_bytes: u64,
    transferred_bytes: u64,
    file_size: u64,
    offset: u64,
    bytes: Vec<u8>,
    message: String,
) -> Message {
    Message::FileTransfer {
        target_id,
        transfer_id,
        direction,
        action,
        path,
        relative_path,
        total_bytes,
        transferred_bytes,
        file_size,
        offset,
        bytes,
        message,
    }
}
