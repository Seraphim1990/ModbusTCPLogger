use std::sync::Arc;
use axum::{extract::State, Json, extract::Path, Router, middleware};
use axum::routing::{get, post, put, delete};
use crate::db::schemas::value_unit::*;
use crate::api::init_axum::AppState;
use axum::response::{IntoResponse, Response};
use axum::http::StatusCode;
use tokio::sync::oneshot;
use crate::logger::printers;
use crate::api::router::handle_get_request::{
    handle_get_request,
    check_send_message
};
use crate::messages::{
    main_msg::MainMsg,
    requests::request_struct::Request,
    requests::value_request::*,
    commands::command::{Command, CommandType},
    commands::value::ValueCommand,
};
use crate::api::router::middlewares::admin_middleware;

pub fn values_router() -> Router<AppState> {
    Router::new()
        .route("/values/get_all", get(get_all_values))
        .route("/values/create", post(create_value))
        .route("/values/get_logging_only", get(get_logging_values))
        .route("/values/get_by_parent_id/{id}", get(get_by_parent_id))
        .route("/values/update/{id}", put(update_value))
        .route("/values/delete/{id}", delete(delete_value))
        .route_layer(middleware::from_fn(admin_middleware))
}

pub async fn get_all_values(State(state): State<AppState>) -> impl IntoResponse {
    let (tx, rx) = oneshot::channel();
    let msg = Request::GetValue(
        ValueRequest::GetAll(
            GetAllValues{
                request_channel: tx
            }
        )
    );
    let request = MainMsg::Request(msg);
    handle_get_request(rx, state.from_api, request).await
}

pub async fn get_logging_values(State(state): State<AppState>) -> impl IntoResponse {
    let (tx, rx) = oneshot::channel();
    let msg = Request::GetValue(
        ValueRequest::GetLoggingOnly(
            GetLoggingOnly{
                request_channel: tx
            }
        )
    );
    let request = MainMsg::Request(msg);
    handle_get_request(rx, state.from_api, request).await
}

pub async fn get_by_parent_id(State(state): State<AppState>, Path(id): Path<u32>) -> impl IntoResponse {
    let (tx, rx) = oneshot::channel();
    let msg = Request::GetValue(
        ValueRequest::GetByDeviceId(
            GetByDeviceId{
                device_id: id as i32,
                request_channel: tx
            }
        )
    );
    let request = MainMsg::Request(msg);
    handle_get_request(rx, state.from_api, request).await
}

pub async fn create_value(State(state): State<AppState>, Json(payload): Json<ValueCreate>) -> impl IntoResponse {
    if payload.value_tag.trim().is_empty() {
        return (StatusCode::BAD_REQUEST, "Тег значення (value_tag) не може бути пустим").into_response();
    }

    let (tx, rx) = oneshot::channel();

    let msg = CommandType::ValueCommand(
        Arc::new(
            ValueCommand::Create(
                ValueCreate{
                    parent_device_id: payload.parent_device_id,
                    value_name: payload.value_name,
                    value_tag: payload.value_tag,
                    description: payload.description,
                    decoding_type: payload.decoding_type,
                    settings: payload.settings,
                    is_logging: payload.is_logging,
                }
            )
        )
    );
    let cmd = MainMsg::Command(
        Command{
            cmd: msg,
            request_channel: tx
        }
    );

    if let Err(e) = check_send_message(&state.from_api, cmd).await {
        return e.into_response();
    }

    match rx.await {
        Ok(Ok(_)) => {(StatusCode::OK, "OK").into_response()},
        Ok(Err(e)) => {
            let msg = format!("Помилка виконання команди створення значення :\n {:?}",e.to_string());
            printers::err(msg.clone());
            (StatusCode::INTERNAL_SERVER_ERROR, msg).into_response()
        },
        Err(e) => {
            let msg = format!("Помилка каналу зворотнього зв'язку при створення значення:\n {:?}", e.to_string());
            printers::err(msg.clone());
            (StatusCode::INTERNAL_SERVER_ERROR, msg).into_response()
        }
    }
}

pub async fn update_value(
    State(state): State<AppState>,
    Path(id): Path<u32>,
    Json(payload): Json<ValueUpdate>,
) -> Response {
    let (tx, rx) = oneshot::channel();
    let msg = CommandType::ValueCommand(
        Arc::new(
            ValueCommand::Update(
                ValueUpdate{
                    id: id as i32,
                    parent_device_id: payload.parent_device_id,
                    value_name: payload.value_name,
                    value_tag: payload.value_tag,
                    description: payload.description,
                    decoding_type: payload.decoding_type,
                    settings: payload.settings,
                    is_logging: payload.is_logging,
                }
            )
        )
    );
    let cmd = MainMsg::Command(
        Command{
            cmd: msg,
            request_channel: tx
        }
    );
    if let Err(e) = check_send_message(&state.from_api, cmd).await {
        return e.into_response();
    }

    match rx.await {
        Ok(Ok(_)) => {(StatusCode::OK, "OK").into_response()},
        Ok(Err(e)) => {
            let msg = format!("Помилка виконання команди оновлення значення :\n {:?}",e.to_string());
            printers::err(msg.clone());
            (StatusCode::INTERNAL_SERVER_ERROR, msg).into_response()
        },
        Err(e) => {
            let msg = format!("Помилка каналу зворотнього зв'язку при оновлення значення:\n {:?}", e.to_string());
            printers::err(msg.clone());
            (StatusCode::INTERNAL_SERVER_ERROR, msg).into_response()
        }
    }
}

pub async fn delete_value(State(state): State<AppState>, Path(id): Path<u32>) -> impl IntoResponse {
    let (tx, rx) = oneshot::channel();
    let msg = CommandType::ValueCommand(
        Arc::new(
            ValueCommand::Delete(
                ValueDelete{
                    id: id as i32
                }
            )
        )
    );
    let cmd = MainMsg::Command(
        Command{
            cmd: msg,
            request_channel: tx
        }
    );
    if let Err(e) = check_send_message(&state.from_api, cmd).await {
        return e.into_response();
    }
    match rx.await {
        Ok(Ok(_)) => {(StatusCode::OK, "OK").into_response()},
        Ok(Err(e)) => {
            let msg = format!("Помилка виконання команди видалення значення :\n {:?}",e.to_string());
            printers::err(msg.clone());
            (StatusCode::INTERNAL_SERVER_ERROR, msg).into_response()
        },
        Err(e) => {
            let msg = format!("Помилка каналу зворотнього зв'язку при видалення значення:\n {:?}", e.to_string());
            printers::err(msg.clone());
            (StatusCode::INTERNAL_SERVER_ERROR, msg).into_response()
        }
    }
}
