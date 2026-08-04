//! WebSocket temps réel : chaque connexion reçoit les événements qui la
//! concernent (nouveaux messages, accusés de lecture). L'envoi passe par
//! l'API REST — le socket ne fait que pousser.

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::Response;
use tokio::sync::broadcast;
use uuid::Uuid;

use crate::extract::CurrentUser;
use crate::AppState;

/// Un événement à pousser aux participants listés.
#[derive(Debug, Clone)]
pub struct WsEvent {
    pub targets: [Uuid; 2],
    pub payload: String,
}

/// Diffuse un événement aux deux participants d'une conversation.
pub fn broadcast_event(state: &AppState, targets: [Uuid; 2], payload: serde_json::Value) {
    // Personne de connecté = personne à prévenir : l'erreur est normale.
    let _ = state.events.send(WsEvent {
        targets,
        payload: payload.to_string(),
    });
}

pub async fn ws_upgrade(
    State(state): State<AppState>,
    user: CurrentUser,
    ws: WebSocketUpgrade,
) -> Response {
    ws.on_upgrade(move |socket| handle_socket(socket, state, user.user_id))
}

async fn handle_socket(mut socket: WebSocket, state: AppState, user_id: Uuid) {
    let mut events = state.events.subscribe();
    loop {
        tokio::select! {
            event = events.recv() => match event {
                Ok(event) if event.targets.contains(&user_id) => {
                    if socket.send(Message::Text(event.payload.into())).await.is_err() {
                        break;
                    }
                }
                Ok(_) => {}
                Err(broadcast::error::RecvError::Lagged(_)) => continue,
                Err(broadcast::error::RecvError::Closed) => break,
            },
            frame = socket.recv() => match frame {
                // Le client n'envoie que des pings de maintien de connexion.
                Some(Ok(Message::Close(_))) | None => break,
                Some(Ok(_)) => {}
                Some(Err(_)) => break,
            },
        }
    }
}
