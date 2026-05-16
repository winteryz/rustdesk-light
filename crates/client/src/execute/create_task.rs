use rdl_protocol::CommandKind;

pub(super) fn handle(payload: &str) -> String {
    format!(
        "TODO: {} accepted as planned stub; payload='{}'",
        CommandKind::CreateTask.as_str(),
        payload
    )
}
