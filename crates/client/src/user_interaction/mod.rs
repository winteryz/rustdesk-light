pub(crate) mod balloon_tip;
pub(crate) mod message_box;
pub(crate) mod open_text_in_notepad;
mod payload;
mod platform;
pub(crate) mod text_chat;
pub(crate) mod voice_chat;

use rdl_protocol::CommandKind;

pub(crate) fn handle(command: &CommandKind, payload: &str, gui_mode: bool) -> String {
    match command {
        CommandKind::TextChat => text_chat::handle(gui_mode),
        CommandKind::MessageBox => message_box::handle(payload, gui_mode),
        CommandKind::BalloonTip => balloon_tip::handle(payload, gui_mode),
        CommandKind::VoiceChat => voice_chat::handle(payload),
        CommandKind::OpenTextInNotepad => open_text_in_notepad::handle(payload, gui_mode),
        _ => format!(
            "TODO: {} accepted as planned stub; payload='{}'",
            command.as_str(),
            payload
        ),
    }
}
