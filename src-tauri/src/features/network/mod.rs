use std::net::SocketAddr;
use std::sync::Arc;

use tokio::sync::broadcast;
use tracing::info;

use crate::features::network::api::{build_router, ApiState};
use crate::features::network::sse::{new_channel, SseEvent};
use crate::features::player::PlayerManager;
use crate::infra::db::Database;

pub mod api;
pub mod sse;

/// Manages the HTTP API server.
///
/// Listens on a configurable address and exposes:
/// - REST endpoints for library access and player control
/// - SSE endpoint for real-time player status
pub struct NetworkManager {
    shutdown_tx: Option<tokio::sync::oneshot::Sender<()>>,
    /// Broadcast channel sender for SSE events.
    pub sse_tx: broadcast::Sender<SseEvent>,
}

impl NetworkManager {
    /// Create a new NetworkManager without starting the server.
    pub fn new() -> Self {
        let (sse_tx, _) = new_channel();
        NetworkManager {
            shutdown_tx: None,
            sse_tx,
        }
    }

    /// Start the HTTP API server on the given address.
    ///
    /// Spawns the server in a background tokio task.
    /// The `player` is shared via `Arc<Mutex>` for thread-safe access.
    pub async fn start(
        &mut self,
        addr: SocketAddr,
        db: Arc<Database>,
        player: Arc<tokio::sync::Mutex<PlayerManager>>,
    ) -> Result<(), std::io::Error> {
        let state = ApiState {
            db,
            player,
            sse_tx: self.sse_tx.clone(),
        };

        let router = build_router(state);
        let listener = tokio::net::TcpListener::bind(addr).await?;
        let local_addr = listener.local_addr()?;

        let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();

        tokio::spawn(async move {
            info!("HTTP API server listening on http://{local_addr}");
            axum::serve(listener, router)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx.await;
                })
                .await
                .unwrap_or_else(|e| tracing::error!("server error: {e}"));
        });

        self.shutdown_tx = Some(shutdown_tx);
        Ok(())
    }

    /// Check if the server is currently running.
    pub fn is_running(&self) -> bool {
        self.shutdown_tx.is_some()
    }

    /// Shut down the HTTP server.
    pub async fn stop(&mut self) {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(());
        }
    }
}

impl Default for NetworkManager {
    fn default() -> Self {
        Self::new()
    }
}
