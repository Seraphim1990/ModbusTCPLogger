use axum::{extract::State, Json, extract::Path, Router, middleware};
use axum::routing::{delete, get, post, put};
use crate::db::schemas::node::*;
use crate::api::init_axum::AppState;
use axum::response::{IntoResponse, Response};
use axum::http::StatusCode;
use crate::logger::printers;
use std::net::IpAddr;
use std::sync::Arc;
use tokio::sync::oneshot;
use crate::messages::{
    main_msg::MainMsg,
    requests::request_struct::Request,
    requests::node_request::*
};
use crate::messages::commands::command::{Command, CommandType};
use crate::messages::commands::node::NodeCommand;
use crate::api::router::handle_get_request::{
    handle_get_request,
    check_send_message
};
use crate::api::router::middlewares::admin_middleware;

pub fn node_router() -> Router<AppState> {
    Router::new()
        .route("/nodes/get_all", get(get_nodes))
        .route("/nodes/get_node_by_ip/{ip}", get(get_node_by_ip))
        .route("/nodes/create", post(create_node))
        .route("/nodes/update/{id}", put(update_node))
        .route("/nodes/delete/{id}", delete(delete_node))
        .route_layer(middleware::from_fn(admin_middleware))
        .route("/nodes/{id}", get(get_node_by_id))
}

pub async fn get_nodes(State(state): State<AppState>) -> impl IntoResponse {
    let (tx, rx) = oneshot::channel();
    let request = MainMsg::Request(
        Request::GetNode(
            NodeRequest::GetAll(
                GetAllNodes{request_channel: tx}
            )
        )
    );
    handle_get_request(rx, state.from_api, request).await
}

pub async fn get_node_by_ip(State(state): State<AppState>, Path(ip): Path<String>) -> impl IntoResponse {
    let (tx, rx) = oneshot::channel();
    let request = create_get_by_ip(&ip, tx);
    handle_get_request(rx, state.from_api, request).await
}

pub async fn get_node_by_id(
    State(state): State<AppState>,
    Path(id): Path<u32>,
) -> impl IntoResponse {
    let (tx, rx) = oneshot::channel();
    let request = MainMsg::Request(
        Request::GetNode(
            NodeRequest::GetById(
                GetById { node_id: id as i32, request_channel: tx, }
            )
        )
    );
    handle_get_request(rx, state.from_api, request).await
}

pub async fn create_node(State(state): State<AppState>, Json(payload): Json<NodeCreate>) -> Response {

    if let Err(e) = check_ip(&payload.ip) {
        return e.into_response();
    }
    let (tx, rx) = oneshot::channel();
    let request = create_get_by_ip(&payload.ip, tx);
    if let Err(e) = check_send_message(&state.from_api, request).await {
        return e.into_response();
    }

    match rx.await {
        Ok(Ok(Some(_))) => {
            let msg = format!("Нода з таким Ip: {} уже існує", &payload.ip);
            printers::err(msg.clone());
            return (StatusCode::INTERNAL_SERVER_ERROR, msg).into_response();
        },
        Ok(Err(_)) => {
            let msg = "Помилка роботи з базою даних, дивись логи".to_string();
            printers::err(msg.clone());
            return (StatusCode::INTERNAL_SERVER_ERROR, msg).into_response()
        },
        Err(e) => {
            let msg = format!("Помилка сервера: {}", e);
            printers::err(msg.clone());
            return (StatusCode::INTERNAL_SERVER_ERROR, msg).into_response();
        },
        Ok(Ok(None)) => {},
    }
    let port = payload.port.unwrap_or(502);
    if !(0..=65535).contains(&port) {
        let msg = format!("Помилка створення ноди:{}, не валідний порт", port);
        printers::err(msg.clone());
        return (StatusCode::INTERNAL_SERVER_ERROR, msg).into_response()
    }

    let (tx_callback, rx_callback) = oneshot::channel();

    let create_node_cmd = Arc::new(  // ужос, блядь, з цими каналами...
                                     NodeCommand::Create(
                                         NodeCreate {ip: payload.ip.clone(),
                                             port: Some(port),
                                             description: payload.description.clone(),
                                         }
                                     )
    );

    let cmd = MainMsg::Command(
        Command {
            cmd: CommandType::NodeCommand(create_node_cmd),
            request_channel: tx_callback
        }
    );

    if let Err(e) = check_send_message(&state.from_api, cmd).await {
        return e.into_response();
    }

    match rx_callback.await {
        Ok(Ok(_)) => (StatusCode::OK, "OK").into_response(),
        Ok(Err(e)) => {
            let msg = format!("Помилка виконання команди створення ноди:\n {:?}", e.to_string());
            printers::err(msg.clone());
            (StatusCode::INTERNAL_SERVER_ERROR, msg).into_response()
        },
        Err(e) => {
            let msg = format!("Помилка каналу зворотнього зв'язку:\n {:?}", e.to_string());
            printers::err(msg.clone());
            (StatusCode::INTERNAL_SERVER_ERROR, msg).into_response()
        }
    }
}

pub async fn update_node(
    State(state): State<AppState>,
    Path(id): Path<u32>,
    Json(payload): Json<NodeUpdate>,
) -> Response {
    // 1. ПЕРЕВІРКА: Чи існує нода, яку ми хочемо оновити?
    let (tx_check, rx_check) = oneshot::channel();
    let get_by_id_req = MainMsg::Request(Request::GetNode(NodeRequest::GetById(GetById {
        node_id: id as i32,
        request_channel: tx_check,
    })));

    if let Err(e) = check_send_message(&state.from_api, get_by_id_req).await {
        return e.into_response();
    }

    match rx_check.await {
        Ok(Ok(Some(_))) => {}
        Ok(Ok(None)) => {
            let msg = format!("Помилка: ноду з ID {} не знайдено", id);
            printers::err(msg.clone());
            return (StatusCode::NOT_FOUND, msg).into_response(); // 404 Клієнту
        }
        Ok(Err(_)) => {
            let msg = "Помилка бази даних при перевірці ноди за ID".to_string();
            printers::err(msg.clone());
            return (StatusCode::INTERNAL_SERVER_ERROR, msg).into_response();
        }
        Err(e) => {
            let msg = format!("Помилка сервера (канал перевірки ID закрився): {}", e);
            printers::err(msg.clone());
            return (StatusCode::INTERNAL_SERVER_ERROR, msg).into_response();
        }
    }

    if let Some(port) = payload.port && !(0..=65535).contains(&port){
        let msg = format!("Помилка оновлення ноди: {}, не валідний порт", port);
        printers::err(msg.clone());
        return (StatusCode::BAD_REQUEST, msg).into_response();
    }

    // 3. ПЕРЕВІРКА IP: Якщо клієнт хоче змінити IP, перевіряємо його валідність та унікальність
    if let Some(ref ip) = payload.ip {
        if let Err(e) = check_ip(ip) {
            return e.into_response();
        }
        let (tx_ip, rx_ip) = oneshot::channel();
        let get_by_ip_req = create_get_by_ip(ip, tx_ip);

        if let Err(e) = check_send_message(&state.from_api, get_by_ip_req).await {
            return e.into_response();
        }

        match rx_ip.await {
            Ok(Ok(Some(existing_node))) => {
                // Якщо IP знайдено, але він належить іншій ноді — це помилка
                if existing_node.id != id as i32 {
                    let msg = format!("Помилка: IP {} вже належить іншій ноді (ID: {})", ip, existing_node.id);
                    printers::err(msg.clone());
                    return (StatusCode::BAD_REQUEST, msg).into_response();
                }
            }
            Ok(Ok(None)) => {} // IP вільний, все ок
            Ok(Err(_)) => {
                let msg = "Помилка бази даних при перевірці унікальності IP".to_string();
                printers::err(msg.clone());
                return (StatusCode::INTERNAL_SERVER_ERROR, msg).into_response();
            }
            Err(e) => {
                let msg = format!("Помилка сервера (канал перевірки IP закрився): {}", e);
                printers::err(msg.clone());
                return (StatusCode::INTERNAL_SERVER_ERROR, msg).into_response();
            }
        }
    }

    let (tx_callback, rx_callback) = oneshot::channel();
    let update_node_cmd = Arc::new(NodeCommand::Update(NodeUpdate {
        id: id as i32, // Гарантуємо, що ID береться з URL шляху запиту
        ip: payload.ip,
        port: payload.port,
        description: payload.description,
    }));

    let cmd = MainMsg::Command(Command {
        cmd: CommandType::NodeCommand(update_node_cmd),
        request_channel: tx_callback,
    });

    if let Err(e) = check_send_message(&state.from_api, cmd).await {
        return e.into_response();
    }

    // 5. ОЧІКУВАННЯ РЕЗУЛЬТАТУ ОНОВЛЕННЯ ВІД АКТОРА
    match rx_callback.await {
        Ok(Ok(_)) => (StatusCode::OK, "OK").into_response(),
        Ok(Err(e)) => {
            let msg = format!("Помилка виконання команди оновлення ноди {}:\n {:?}", id, e.to_string());
            printers::err(msg.clone());
            (StatusCode::INTERNAL_SERVER_ERROR, msg).into_response()
        },
        Err(e) => {
            let msg = format!("Помилка каналу зворотнього зв'язку при оновленні:\n {:?}", e.to_string());
            printers::err(msg.clone());
            (StatusCode::INTERNAL_SERVER_ERROR, msg).into_response()
        }
    }
}

pub async fn delete_node(State(state): State<AppState>, Path(id): Path<u32>) -> Response {
    let (tx_callback, rx_callback) = oneshot::channel();

    let delete_node_cmd = Arc::new(NodeCommand::Delete(NodeDelete { id: id as i32 }));

    let cmd = MainMsg::Command(Command {
        cmd: CommandType::NodeCommand(delete_node_cmd),
        request_channel: tx_callback,
    });

    if let Err(e) = check_send_message(&state.from_api, cmd).await {
        return e.into_response();
    }

    match rx_callback.await {
        Ok(Ok(_)) => (StatusCode::OK, "OK").into_response(),
        Ok(Err(e)) => {
            let msg = format!("Помилка виконання команди видалення ноди {}:\n {:?}", id, e.to_string());
            printers::err(msg.clone());
            (StatusCode::INTERNAL_SERVER_ERROR, msg).into_response()
        },
        Err(e) => {
            let msg = format!("Помилка каналу зворотнього зв'язку при видаленні:\n {:?}", e.to_string());
            printers::err(msg.clone());
            (StatusCode::INTERNAL_SERVER_ERROR, msg).into_response()
        }
    }
}
#[allow(clippy::result_large_err)]
fn check_ip(ip: &String) -> Result<(), impl IntoResponse> {
    if ip.parse::<IpAddr>().is_err() {
        let msg = format!("Невірний ip: {}", ip);
        printers::err(msg.clone());
        return Err((StatusCode::INTERNAL_SERVER_ERROR, msg).into_response());
    }
    Ok(())
}

fn create_get_by_ip(ip: &str, tx: oneshot::Sender<Result<Option<NodeRead>, ()>>) -> MainMsg {
    MainMsg::Request(
        Request::GetNode(
            NodeRequest::GetByIp(
                GetByIp{node_ip: ip.to_string(), request_channel: tx}
            )
        )
    )
}