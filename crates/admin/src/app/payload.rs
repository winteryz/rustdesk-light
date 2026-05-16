pub(super) fn video_stream_payload(payload: &str) -> bool {
    payload
        .lines()
        .find_map(|line| line.strip_prefix("action="))
        .map(|action| matches!(action.trim(), "start" | "stop"))
        .unwrap_or(false)
}

pub(super) fn payload_field(payload: &str, key: &str) -> Option<String> {
    let prefix = format!("{key}=");
    payload
        .lines()
        .find_map(|line| line.strip_prefix(&prefix))
        .map(|value| value.trim().to_string())
}
