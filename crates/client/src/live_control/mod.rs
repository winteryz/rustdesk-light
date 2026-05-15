use rdl_protocol::CommandKind;

mod camera;
mod remote_desktop;

pub fn handle(command: &CommandKind, payload: &str) -> String {
    match command {
        CommandKind::RemoteDesktop => remote_desktop::handle(payload),
        CommandKind::Camera => camera::handle(payload),
        _ => format!(
            "TODO: {} accepted as planned stub; payload='{}'",
            command.as_str(),
            payload
        ),
    }
}
