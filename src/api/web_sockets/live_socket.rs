use std::ops::Deref;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    response::IntoResponse,
    routing::get,
    Router,
};
use crate::logger::printers;
use std::time::Duration;
use axum::extract::Query;
use axum::extract::ws::Utf8Bytes;
use axum::http::StatusCode;
use jsonwebtoken::{decode, DecodingKey, Validation};
use serde::Deserialize;
use tokio::time::{sleep, timeout};
use std::sync::atomic::Ordering;



use crate::api::init_axum::{AppState, Claims, JWT_KEY};
use tokio::sync::mpsc;
use crate::api::web_sockets::live_socket_unit::{CoordUnitWebSocketCommand, CoordUnitWebSocketData};


pub fn live_router() -> Router<AppState> {
    Router::new()
        .route("/live_data", get(ws_handler))
}

#[derive(Deserialize)]
pub struct WsParams {
    token: Option<String>,
}

async fn ws_handler(
    ws: WebSocketUpgrade,
    Query(params): Query<WsParams>, // Axum автоматично дістає ?token= із запиту
    State(state): State<AppState>,
) -> impl IntoResponse {
    if let Some(token_str) = params.token && decode::<Claims>(
            token_str,
            &DecodingKey::from_secret(JWT_KEY),
            &Validation::default(), // Перевіряє exp і iat автоматично
        )
        .is_err() {
            return StatusCode::UNAUTHORIZED.into_response();
        }
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    let mut receiver;
    let mut current_id: usize;
    let duration = Duration::from_secs(30);

    match timeout(duration, socket.recv()).await {
        Ok(read_some) => {
            match read_some {
                Some(Ok(Message::Text(config))) => {
                    current_id = state.ws_counter.deref().fetch_add(1, Ordering::Relaxed);
                    if let Ok(rec) = new_config(&mut socket, &state.to_ws_coord, config.to_string(), current_id).await {
                        receiver = rec;
                    } else {
                        printers::warn("Помилка повідомлення вебсокету".to_string());
                        return
                    }
                },
                _ => {
                    printers::warn("Падіння вебсокету".to_string());
                    return
                },
            }
        }
        Err(_) => {
            printers::warn("Таймаут по підключенні вебсокету".to_string());
            return;
        }
    }

    let timer = sleep(Duration::from_secs(900));
    tokio::pin!(timer);
    loop {
        tokio::select! {
            msg = socket.recv() => {
                match msg {
                    Some(Ok(Message::Text(config))) => {
                        let old_id = current_id;
                        current_id = state.ws_counter.deref().fetch_add(1, Ordering::Relaxed);
                        if let Ok(rec) = change_config(&mut socket, &state.to_ws_coord, config.to_string(), current_id, old_id).await {
                            receiver = rec;
                        } else {
                            printers::warn("Помилка повідомлення вебсокету".to_string());
                            break;
                        }
                    },
                    _ => {
                        break;
                    },
                }
            }
            event = receiver.recv() => {
                match event {
                    Some(message) => {
                        if let Err(e) = socket.send(Message::Text(Utf8Bytes::from(message))).await {
                            printers::err(format!("Помилка відправки подій в вебсокет: {:?}", e));
                            break;
                        }
                    },
                    None => break, // канал упав
                }
            }
            _ = &mut timer => { // розрив зьєднання для отримання нового токену і перепідключення
                break;
            }
        }
    }
    if let Err(_) = delete_config(&state.to_ws_coord, current_id).await {
        printers::err("Помилка чистки вебсокету".to_string());
    }
}

async fn delete_config(to_ws_coord: &mpsc::Sender<CoordUnitWebSocketCommand>,
                       id: usize) -> Result<(), ()> {

    let unit = CoordUnitWebSocketCommand::Delete{ id };

    if let Err(e) = send_msg(None, to_ws_coord, unit).await {
        return Err(e);
    }

    Ok(())
}

async fn change_config(websocket: &mut WebSocket,
                       to_ws_coord: &mpsc::Sender<CoordUnitWebSocketCommand>,
                       config: String,
                       new_id: usize,
                       old_id: usize) -> Result<mpsc::Receiver<String>, ()> {
    let (sender, receiver) = mpsc::channel(2);
    let send_unit = CoordUnitWebSocketData::new(config.as_str(), sender).map_err(|e| {
        printers::warn(format!("Невалідна конфігурація вебсокету: {:?}", e));
    })?;

    let unit = CoordUnitWebSocketCommand::Change{ old_id, new_id, unit: send_unit };
    if let Err(_) = send_msg(Some(websocket), to_ws_coord, unit).await {
        return Err(())
    };

    Ok(receiver)
}

async fn new_config(websocket: &mut WebSocket, to_ws_coord: &mpsc::Sender<CoordUnitWebSocketCommand>, config: String, id: usize) -> Result<mpsc::Receiver<String>, ()> {
    let (sender, receiver) = mpsc::channel(2);

    let send_unit = CoordUnitWebSocketData::new(config.as_str(), sender).map_err(|e| {
        printers::warn(format!("Невалідна конфігурація вебсокету: {:?}", e));
    })?;
    let unit = CoordUnitWebSocketCommand::New{ id, unit: send_unit };

    if let Err(_) = send_msg(Some(websocket), to_ws_coord, unit).await {
        return Err(())
    };
    Ok(receiver)
}

async fn send_msg(websocket: Option<&mut WebSocket>, to_ws_coord: &mpsc::Sender<CoordUnitWebSocketCommand>, unit: CoordUnitWebSocketCommand) -> Result<(), ()> {
    if let Err(e) = to_ws_coord.send(unit).await {
        let msg = format!("Помилка відправки конфігурації вебсокету: {:?}", e);
        printers::err(msg.clone());

        if let Some(websocket) = websocket &&
            let Err(e) = websocket.send(Message::Text(Utf8Bytes::from(msg))).await {
            printers::err(format!("Помилка відправки зворотнього звязку вебсокету: {:?}", e));
        }
        return Err(())
    }
    Ok(())
}