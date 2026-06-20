use std::convert::Infallible;
use std::time::Duration;

use axum::response::sse::{Event, KeepAlive, Sse};
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

use crate::features::player::PlaybackStatus;

/// An event sent over SSE to connected clients.
#[derive(Debug, Clone)]
pub struct SseEvent {
    pub event_type: String,
    pub data: serde_json::Value,
}

/// Default broadcast channel capacity.
const CHANNEL_CAPACITY: usize = 64;

/// Create a new broadcast channel for SSE events.
pub fn new_channel() -> (broadcast::Sender<SseEvent>, broadcast::Receiver<SseEvent>) {
    broadcast::channel(CHANNEL_CAPACITY)
}

/// Publish a playback status update to all SSE listeners.
pub fn publish_status(tx: &broadcast::Sender<SseEvent>, status: &PlaybackStatus) {
    if let Ok(data) = serde_json::to_value(status) {
        let _ = tx.send(SseEvent {
            event_type: "player_status".to_string(),
            data,
        });
    }
}

/// Build the SSE response stream from a broadcast receiver.
pub fn sse_stream(
    rx: broadcast::Receiver<SseEvent>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    let stream = BroadcastStream::new(rx).filter_map(|result| match result {
        Ok(event) => {
            let data = serde_json::to_string(&event.data).unwrap_or_default();
            Some(Ok(Event::default().event(event.event_type).data(data)))
        }
        Err(_) => None,
    });

    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}
