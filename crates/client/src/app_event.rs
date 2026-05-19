use rdl_protocol::CommandKind;
use std::sync::mpsc::Sender;

#[cfg(feature = "gui")]
use eframe::egui;
#[cfg(feature = "gui")]
use std::sync::{Arc, Mutex};

#[derive(Debug)]
pub(crate) enum ClientEvent {
    Connected,
    Disconnected,
    Command {
        command: CommandKind,
        payload: String,
    },
    ChatMessage {
        text: String,
    },
    VoiceChatInvite,
    VoiceChatConnected,
    VoiceChatEnded {
        message: String,
    },
    VoiceChatFailed {
        message: String,
    },
    Log(String),
}

#[cfg_attr(not(feature = "gui"), allow(dead_code))]
pub(crate) enum ClientInput {
    ChatReply { text: String },
    VoiceChatAccept,
    VoiceChatDecline,
    VoiceChatEnd,
    VoiceChatMicMuted { muted: bool },
    VoiceChatSpeakerMuted { muted: bool },
}

#[derive(Clone)]
pub(crate) struct ClientEventSink {
    tx: Sender<ClientEvent>,
    #[cfg(feature = "gui")]
    repaint_handle: Option<Arc<Mutex<Option<egui::Context>>>>,
}

impl ClientEventSink {
    #[cfg(feature = "gui")]
    pub(crate) fn new(
        tx: Sender<ClientEvent>,
        repaint_handle: Option<Arc<Mutex<Option<egui::Context>>>>,
    ) -> Self {
        Self { tx, repaint_handle }
    }

    #[cfg(not(feature = "gui"))]
    pub(crate) fn new(tx: Sender<ClientEvent>, _repaint_handle: Option<()>) -> Self {
        Self { tx }
    }

    pub(crate) fn send(&self, event: ClientEvent) {
        let _ = self.tx.send(event);
        #[cfg(feature = "gui")]
        if let Some(ctx) = self
            .repaint_handle
            .as_ref()
            .and_then(|handle| handle.lock().ok().and_then(|ctx| ctx.clone()))
        {
            ctx.request_repaint_of(egui::ViewportId::ROOT);
        }
    }
}
