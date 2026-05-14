use rdl_protocol::CommandKind;

pub fn handle(command: &CommandKind, payload: &str, gui_mode: bool) -> String {
    match command {
        CommandKind::MessageBox | CommandKind::BalloonTip | CommandKind::TextChat => {
            if gui_mode {
                format!("shown in client gui log: {payload}")
            } else {
                println!("admin message: {payload}");
                "shown in terminal fallback".to_string()
            }
        }
        _ => format!(
            "TODO: {} accepted as planned stub; payload='{}'",
            command.as_str(),
            payload
        ),
    }
}
