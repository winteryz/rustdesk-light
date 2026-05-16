use rdl_protocol::CommandKind;

pub(super) fn handle(payload: &str) -> String {
    format!(
        "TODO: {} accepted as planned stub; payload='{}'",
        CommandKind::CommandPreset.as_str(),
        payload
    )
}
