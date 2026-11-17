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
                    Some(Ok(Message::Text(config))) if config.as_str() == "ping" => {
                        if socket.send(Message::Text(Utf8Bytes::from_static("pong"))).await.is_err() {
                            break;
                        }
                    },
                    Some(Ok(Message::Ping(_))) | Some(Ok(Message::Pong(_))) => {},
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
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, atomic::AtomicUsize};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::{TcpListener, TcpStream};

    async fn send_frame(client: &mut TcpStream, opcode: u8, payload: &[u8]) {
        assert!(payload.len() < 126);
        let mask = [1, 2, 3, 4];
        let mut frame = vec![0x80 | opcode, 0x80 | payload.len() as u8];
        frame.extend_from_slice(&mask);
        for (i, byte) in payload.iter().enumerate() {
            frame.push(byte ^ mask[i % 4]);
        }
        client.write_all(&frame).await.unwrap();
    }

    async fn read_frame(client: &mut TcpStream) -> (u8, Vec<u8>) {
        let mut header = [0; 2];
        client.read_exact(&mut header).await.unwrap();
        assert_eq!(header[1] & 0x80, 0);
        let length = match header[1] & 0x7f {
            126 => client.read_u16().await.unwrap() as usize,
            127 => client.read_u64().await.unwrap() as usize,
            length => length as usize,
        };
        assert!(length < 65536);
        let mut payload = vec![0; length];
        client.read_exact(&mut payload).await.unwrap();
        (header[0] & 0x0f, payload)
    }

    #[tokio::test]
    async fn heartbeat_preserves_subscription_and_control_ping_keeps_socket_open() {
        timeout(Duration::from_secs(5), async {
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let (to_coord, mut from_socket) = mpsc::channel(10);
            let (to_api, _from_api) = mpsc::channel(10);
            let state = AppState {
                from_api: to_api,
                to_ws_coord: to_coord,
                ws_counter: Arc::new(AtomicUsize::new(0)),
            };
            let server = tokio::spawn(async move {
                axum::serve(listener, live_router().with_state(state)).await.unwrap();
            });
            let mut client = TcpStream::connect(address).await.unwrap();
            client.write_all(format!(
                "GET /live_data HTTP/1.1\r\nHost: {}\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Key: dGhlIHNhbXBsZSBub25jZQ==\r\nSec-WebSocket-Version: 13\r\n\r\n", address
            ).as_bytes()).await.unwrap();
            let mut response = Vec::new();
            while !response.ends_with(b"\r\n\r\n") {
                response.push(client.read_u8().await.unwrap());
                assert!(response.len() < 4096);
            }
            assert!(response.starts_with(b"HTTP/1.1 101"));
            send_frame(&mut client, 1, br#"{"values":["sensor"],"nodes":[],"devices":[]}"#).await;
            let Some(CoordUnitWebSocketCommand::New { id, mut unit }) = from_socket.recv().await
                else { panic!("expected subscription") };
            send_frame(&mut client, 1, b"ping").await;
            assert_eq!(read_frame(&mut client).await, (1, b"pong".to_vec()));
            send_frame(&mut client, 9, b"control").await;
            assert_eq!(read_frame(&mut client).await, (10, b"control".to_vec()));
            unit.value_events(Arc::new("sensor".into()), 42.0);
            assert!(unit.flush());
            let (opcode, payload) = read_frame(&mut client).await;
            assert_eq!(opcode, 1);
            let event: serde_json::Value = serde_json::from_slice(&payload).unwrap();
            assert_eq!(event["values"][0]["tag"], "sensor");
            assert_eq!(event["values"][0]["value"], 42.0);
            assert!(from_socket.try_recv().is_err());
            drop(client);
            let Some(CoordUnitWebSocketCommand::Delete { id: removed }) = from_socket.recv().await
                else { panic!("expected subscription cleanup") };
            assert_eq!(removed, id);
            server.abort();
        }).await.expect("WebSocket heartbeat test timed out");
    }
}
