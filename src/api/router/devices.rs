use std::sync::Arc;
use axum::{extract::State, Json, extract::Path, Router, middleware};
use axum::routing::{get, post, put, delete};
use crate::db::schemas::device::*;
use axum::response::{IntoResponse, Response};
use axum::http::StatusCode;
use serde::Serialize;
use tokio::sync::{mpsc, oneshot};
use crate::logger::printers;
use crate::api::init_axum::AppState;
use crate::messages::main_msg::MainMsg;
use crate::messages::requests::node_request::{GetById, NodeRequest};
use crate::messages::requests::request_struct::Request;
use crate::messages::commands::{
    command::{Command, CommandType},
    device::DeviceCommand
};
use crate::messages::requests::device_request::{DeviceRequest, GetAllDevices, GetDeviceByNode};
use crate::messages::requests::device_request::*;
use crate::api::router::handle_get_request::{
    handle_get_request,
    check_send_message
};
use crate::api::router::middlewares::admin_middleware;

pub fn devices_router() -> Router<AppState> {
    Router::new()
        .route("/devices/get_all", get(get_devices))
        .route("/devices/get_by_parent_id/{id}", get(get_devices_by_parent_id))
        .route("/devices/create", post(create_devices))
        .route("/devices/update/{id}", put(update_device))
        .route("/devices/delete/{id}", delete(delete_device))
        .route_layer(middleware::from_fn(admin_middleware))
        .route("/devices/get_device/{id}", get(get_device))
}

pub async fn create_devices(State(state): State<AppState>, Json(payload): Json<DeviceCreate>) -> impl IntoResponse {
    // Валідація вхідних даних
    if payload.address < 0 || payload.address > 255 {
        return (StatusCode::BAD_REQUEST, "Не вірна адреса пристрою").into_response();
    }
    if payload.time_for_recall <= 0 {
        return (StatusCode::BAD_REQUEST, "Час опитування не може бути від'ємним або нулем").into_response();
    }
    if payload.retry_count <= 0 {
        return (StatusCode::BAD_REQUEST, "Кількість повторів не може бути від'ємною або нулем").into_response();
    }
    if payload.timeout <= 0 {
        return (StatusCode::BAD_REQUEST, "Час таймауту (timeout) не може бути від'ємним або нулем").into_response();
    }
    if payload.parent_node_id <= 0 {
        return (StatusCode::BAD_REQUEST, "id Батьківської ноди не може бути від'ємним").into_response();
    }

    let (tx, rx) = oneshot::channel();
    let request = MainMsg::Request(
        Request::GetNode(
            NodeRequest::GetById(
                GetById { node_id: payload.parent_node_id, request_channel: tx, }
            )
        )
    );

    if let Err(e) = check_send_message(&state.from_api, request).await {
        return e.into_response();
    }

    match rx.await {
        Ok(Ok(node)) => {
            if node.is_none() {
                let msg = format!("Відсутня нода id: \n{}",  payload.parent_node_id);
                printers::err(msg.clone());
                return (StatusCode::BAD_REQUEST, msg).into_response()
            }
        },
        Ok(Err(_)) => {
            let msg = "Помилка читання бази данних, детальніше в логах".to_string();
            printers::err(msg.clone());
            return (StatusCode::INTERNAL_SERVER_ERROR, msg).into_response()
        },
        Err(e) => {
            let msg = format!("Помилка каналу: \n{}", e);
            printers::err(msg.clone());
            return (StatusCode::INTERNAL_SERVER_ERROR, msg).into_response()
        }
    }

    let (tx, rx) = oneshot::channel();

    let create_device_cmd = Arc::new(
        DeviceCommand::Create(
            DeviceCreate {
                device_name: payload.device_name,
                address: payload.address,
                parent_node_id: payload.parent_node_id,
                time_for_recall: payload.time_for_recall,
                timeout: payload.timeout,
                retry_count: payload.retry_count,
                is_active: payload.is_active,
                read_by_group: payload.read_by_group,
                description: payload.description,
            }
        )
    );

    let cmd = MainMsg::Command(
        Command {
            cmd: CommandType::DeviceCommand(create_device_cmd),
            request_channel: tx,
        }
    );

    if let Err(e) = check_send_message(&state.from_api, cmd).await {
        return e.into_response();
    }



    match rx.await {
        Ok(Ok(_)) => {(StatusCode::OK, "OK").into_response()},
        Ok(Err(e)) => {
            let msg = format!("Помилка виконання команди створення пристрою:\n {:?}", e.to_string());
            printers::err(msg.clone());
            (StatusCode::INTERNAL_SERVER_ERROR, msg).into_response()
        },
        Err(e) => {
            let msg = format!("Помилка каналу: \n{}", e);
            printers::err(msg.clone());
            (StatusCode::INTERNAL_SERVER_ERROR, msg).into_response()
        }
    }
}

pub async fn get_devices(State(state): State<AppState>) -> impl IntoResponse {

    let (tx, rx) = oneshot::channel();

    let request_body = MainMsg::Request(
        Request::GetDevice(
            DeviceRequest::GetAllDevices(
                GetAllDevices{ request_channel: tx}
            )
        )
    );
    handle_get_request(rx, state.from_api, request_body).await
}

pub async fn get_devices_by_parent_id(
    State(state): State<AppState>,
    Path(parent_id): Path<u32>,
) -> impl IntoResponse {

    let (tx, rx) = oneshot::channel();

    let request_body = MainMsg::Request(
        Request::GetDevice(
            DeviceRequest::GetByNode(
                GetDeviceByNode{
                    node_id: parent_id as i32,
                    request_channel: tx
                }
            )
        )
    );
    handle_get_request(rx, state.from_api, request_body).await;
}

pub async fn get_device(State(state): State<AppState>, Path(id): Path<u32>) -> impl IntoResponse {
    let (tx, rx) = oneshot::channel();

    let request_body = MainMsg::Request(
        Request::GetDevice(
            DeviceRequest::GetDeviceById(
                GetDeviceById{
                    id: id as i32,
                    request_channel: tx
                }
            )
        )
    );

    if let Err(e) = check_send_message(&state.from_api, request_body).await {
        return e.into_response();
    }
    match rx.await {
        Ok(Ok(device)) => {
            match device {
                Some(device) => (StatusCode::OK, Json(device)).into_response(),
                None => (StatusCode::NOT_FOUND, "Пристрій не знайдено").into_response(),
            }
        },
        Ok(Err(e)) => {
            let msg = format!("Помилка виконання команди читання пристрою:\n {:?}", e);
            printers::err(msg.clone());
            (StatusCode::INTERNAL_SERVER_ERROR, msg).into_response()
        },
        Err(e) => {
            let msg = format!("Помилка каналу: \n{}", e);
            printers::err(msg.clone());
            (StatusCode::INTERNAL_SERVER_ERROR, msg).into_response()
        }
    }
}

pub async fn update_device(State(state): State<AppState>, Path(id): Path<u32>, Json(payload): Json<DeviceUpdate>) -> impl IntoResponse {
    
    // Валідація Option-полів безпечним способом
    if let Some(addr) = payload.address && !(0..=255).contains(&addr) {
        return (StatusCode::BAD_REQUEST, "Не вірна адреса пристрою").into_response();
    }
    if let Some(recall) = payload.time_for_recall && recall < 0 {
        return (StatusCode::BAD_REQUEST, "Час опитування не може бути від'ємним").into_response();
    }
    if let Some(timeout) = payload.timeout && timeout < 0 {
        return (StatusCode::BAD_REQUEST, "Час таймауту не може бути від'ємним").into_response();
    }
    if let Some(retry) = payload.retry_count && retry < 0 {
        return (StatusCode::BAD_REQUEST, "Кількість повторів не може бути від'ємною").into_response();
    }

    let update_device_cmd = CommandType::DeviceCommand(
        Arc::new(
            DeviceCommand::Update(
                DeviceUpdate {
                    id: id as i32,
                    device_name: payload.device_name,
                    address: payload.address,
                    parent_node_id: payload.parent_node_id,
                    time_for_recall: payload.time_for_recall,
                    timeout: payload.timeout,
                    retry_count: payload.retry_count,
                    is_active: payload.is_active,
                    read_by_group: payload.read_by_group,
                    description: payload.description,
                }
            )
        )
    );
    let (tx, rx) = oneshot::channel();
    let cmd = MainMsg::Command(
        Command {
            cmd: update_device_cmd,
            request_channel: tx
        }
    );
    handle_change_request(rx, state.from_api, cmd).await
}
pub async fn delete_device(State(state): State<AppState>, Path(id): Path<u32>) -> impl IntoResponse {

    let cmd = CommandType::DeviceCommand(
        Arc::new(
            DeviceCommand::Delete(
                DeviceDelete {
                    id: id as i32,
                }
            )
        )
    );
    let (tx, rx) = oneshot::channel();
    let cmd = MainMsg::Command(
        Command {
             cmd,
             request_channel: tx
         }
    );
    handle_change_request(rx, state.from_api, cmd).await
}

async fn handle_change_request<T>(rx: oneshot::Receiver<Result<T, String>>,
                               send_chanel: mpsc::Sender<MainMsg>,
                               cmd: MainMsg) -> Response
where T: Serialize
{
    if let Err(e) = check_send_message(&send_chanel, cmd).await {
        return e.into_response();
    }
    match rx.await {
        Ok(Ok(_)) => {(StatusCode::OK, "OK").into_response()},
        Ok(Err(e)) => {
            let msg = format!("Помилка виконання команди пристрою:\n {:?}", e);
            printers::err(msg.clone());
            (StatusCode::INTERNAL_SERVER_ERROR, msg).into_response()
        },
        Err(e) => {
            let msg = format!("Помилка каналу: \n{}", e);
            printers::err(msg.clone());
            (StatusCode::INTERNAL_SERVER_ERROR, msg).into_response()
        }
    }
}