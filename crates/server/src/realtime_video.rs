use std::sync::{mpsc, Arc, Mutex};

pub(crate) struct RealtimeVideoSender<T> {
    shared: Arc<RealtimeVideoShared<T>>,
}

pub(crate) struct RealtimeVideoReceiver<T> {
    shared: Arc<RealtimeVideoShared<T>>,
}

struct RealtimeVideoShared<T> {
    state: Mutex<RealtimeVideoState<T>>,
}

struct RealtimeVideoState<T> {
    value: Option<T>,
    sender_count: usize,
    receiver_alive: bool,
}

pub(crate) fn latest_video_channel<T>() -> (RealtimeVideoSender<T>, RealtimeVideoReceiver<T>) {
    let shared = Arc::new(RealtimeVideoShared {
        state: Mutex::new(RealtimeVideoState {
            value: None,
            sender_count: 1,
            receiver_alive: true,
        }),
    });
    (
        RealtimeVideoSender {
            shared: shared.clone(),
        },
        RealtimeVideoReceiver { shared },
    )
}

impl<T> Clone for RealtimeVideoSender<T> {
    fn clone(&self) -> Self {
        if let Ok(mut state) = self.shared.state.lock() {
            state.sender_count = state.sender_count.saturating_add(1);
        }
        Self {
            shared: self.shared.clone(),
        }
    }
}

impl<T> std::fmt::Debug for RealtimeVideoSender<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RealtimeVideoSender")
            .finish_non_exhaustive()
    }
}

impl<T> Drop for RealtimeVideoSender<T> {
    fn drop(&mut self) {
        let Ok(mut state) = self.shared.state.lock() else {
            return;
        };
        state.sender_count = state.sender_count.saturating_sub(1);
    }
}

impl<T> RealtimeVideoSender<T> {
    pub(crate) fn send_latest(&self, value: T) -> Result<(), mpsc::SendError<T>> {
        let Ok(mut state) = self.shared.state.lock() else {
            return Err(mpsc::SendError(value));
        };
        if !state.receiver_alive {
            return Err(mpsc::SendError(value));
        }
        state.value = Some(value);
        Ok(())
    }
}

impl<T> Drop for RealtimeVideoReceiver<T> {
    fn drop(&mut self) {
        let Ok(mut state) = self.shared.state.lock() else {
            return;
        };
        state.receiver_alive = false;
        state.value = None;
    }
}

impl<T> RealtimeVideoReceiver<T> {
    pub(crate) fn try_recv(&self) -> Result<T, mpsc::TryRecvError> {
        let Ok(mut state) = self.shared.state.lock() else {
            return Err(mpsc::TryRecvError::Disconnected);
        };
        if let Some(value) = state.value.take() {
            return Ok(value);
        }
        if state.sender_count == 0 {
            Err(mpsc::TryRecvError::Disconnected)
        } else {
            Err(mpsc::TryRecvError::Empty)
        }
    }
}
