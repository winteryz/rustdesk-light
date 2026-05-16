use rdl_protocol::CommandKind;

pub(crate) fn handle(payload: &str) -> String {
    format!(
        "TODO: {} accepted as planned stub; payload='{}'",
        CommandKind::VoiceChat.as_str(),
        payload
    )
}
