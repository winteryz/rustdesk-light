#[cfg(feature = "gui")]
pub(crate) mod balloon_tip;
#[cfg(feature = "gui")]
pub(crate) mod message_box;
#[cfg(feature = "gui")]
pub(crate) mod open_text_in_notepad;
#[cfg(feature = "gui")]
mod payload;
#[cfg(feature = "gui")]
mod platform;
#[cfg(feature = "gui")]
pub(crate) mod text_chat;
#[cfg(feature = "gui")]
pub(crate) mod voice_chat;

use rdl_protocol::CommandKind;

#[cfg(feature = "gui")]
pub(crate) fn handle(command: &CommandKind, payload: &str, gui_mode: bool) -> String {
    if !gui_mode {
        return disabled_detail(command);
    }

    match command {
        CommandKind::TextChat => text_chat::handle(gui_mode),
        CommandKind::MessageBox => message_box::handle(payload, gui_mode),
        CommandKind::BalloonTip => balloon_tip::handle(payload, gui_mode),
        CommandKind::VoiceChat => voice_chat::handle(payload, gui_mode),
        CommandKind::OpenTextInNotepad => open_text_in_notepad::handle(payload, gui_mode),
        _ => format!(
            "TODO: {} accepted as planned stub; payload='{}'",
            command.as_str(),
            payload
        ),
    }
}

#[cfg(not(feature = "gui"))]
pub(crate) fn handle(command: &CommandKind, _payload: &str, _gui_mode: bool) -> String {
    disabled_detail(command)
}

pub(crate) fn disabled_detail(command: &CommandKind) -> String {
    format!(
        "{}_disabled\nmessage=client GUI is not available",
        command.as_str()
    )
}
