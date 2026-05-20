mod create_task;
mod execute_code;
mod execute_file;
mod execute_static_command;
mod shared;

use rdl_protocol::CommandKind;

pub fn handle(command: &CommandKind, payload: &str) -> String {
    match command {
        CommandKind::ExecuteFile => execute_file::handle(payload),
        CommandKind::ExecuteCode => execute_code::handle(payload),
        CommandKind::ExecuteStaticCommand => execute_static_command::handle(payload),
        CommandKind::CreateTask => create_task::handle(payload),
        _ => format!(
            "TODO: {} accepted as planned stub; payload='{}'",
            command.as_str(),
            payload
        ),
    }
}
