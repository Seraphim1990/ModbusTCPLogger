use std::sync::Arc;
use crate::api::init_axum::AppState;
use axum::{extract::State, Json, extract::Path, Router, middleware};
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};
use crate::db::schemas::users::{UserCreate, UserUpdate, UserDelete};
use crate::messages::main_msg::MainMsg;
use crate::messages::requests::{
    request_struct::Request,
    user_request::*
};
use tokio::sync::oneshot;
use axum::http::StatusCode;
use axum::routing::{delete, get, post, put};
use crate::api::router::handle_get_request::{check_send_message};
use crate::api::router::middlewares::admin_middleware;
use crate::messages::commands::command::{
    CommandType, Command
};
use crate::messages::commands::users;

pub fn users_router() -> Router<AppState> {
    Router::new()
        .route("/users/create", post(create_user))
        .route("/users/get_all", get(get_all_users))
        .route("/users/get_by_id/{id}", get(get_user_by_id))
        .route("/users/update/{id}", put(update_user))
        .route("/users/delete/{id}", delete(delete_user))
        .route_layer(middleware::from_fn(admin_middleware))
}

#[derive(Serialize, Deserialize)]
pub struct UserSendRequest{
    pub id:i32,
    pub name:String,
    pub role_id: i32
}

async fn get_user_by_id(State(state): State<AppState>, Path(id): Path<i32>) -> impl IntoResponse {
    let (tx, rx) = oneshot::channel();
    let msg = UserRequest::GetById(
        UserGetById{
            id,
            request_channel: tx
        }
    );
    let request = MainMsg::Request(
        Request::GetUser(msg)
    );

    if let Err(response) = check_send_message(&state.from_api, request).await {
        return response.into_response();
    }
    let res = rx.await;
    match res {
        Ok(Ok(user)) => {
            if let Some(user) = user{
                (StatusCode::OK, Json(UserSendRequest{id: user.id, name: user.username, role_id: user.role_id})).into_response()
            } else {
                (StatusCode::NOT_FOUND, "User not found").into_response()
            }
        },
        Ok(Err(_)) => (StatusCode::INTERNAL_SERVER_ERROR, "Error from database").into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response()
    }
}

async fn get_all_users(State(state): State<AppState>) -> impl IntoResponse {
    let (tx, rx) = oneshot::channel();
    let msg = UserRequest::GetAll(
        UserGetAll{
            request_channel: tx
        }
    );
    let request = MainMsg::Request(
        Request::GetUser(msg)
    );
    if let Err(response) = check_send_message(&state.from_api, request).await {
        return response.into_response();
    }
    let res = rx.await;
    match res {
        Ok(Ok(users)) => {
            let users = users.into_iter().map(|user|UserSendRequest{id: user.id, name: user.username, role_id: user.role_id}).collect::<Vec<_>>();
            (StatusCode::OK, Json(users)).into_response()
        },
        Ok(Err(_)) => {
            (StatusCode::INTERNAL_SERVER_ERROR, "Error from database").into_response()
        },
        Err(_) => {
            (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response()
        }
    }
}

async fn create_user(State(state): State<AppState>, Json(payload): Json<UserCreate>) -> impl IntoResponse {
    let (tx, rx) = oneshot::channel();
    let msg = CommandType::UserCommand(
        Arc::new(
            users::UserCommand::Create(
                UserCreate {
                    username: payload.username,
                    password_hash: payload.password_hash,
                    role_id: payload.role_id
                }
            )
        )
    );
    let cmd = Command { cmd: msg, request_channel: tx };
    let msg = MainMsg::Command(cmd);
    if let Err(response) = check_send_message(&state.from_api, msg).await {
        return response.into_response();
    }
    let res = rx.await;
    match res {
        Ok(Ok(())) => (StatusCode::OK, "User created").into_response(),
        Ok(Err(err_msg)) => (StatusCode::INTERNAL_SERVER_ERROR, err_msg).into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response()
    }
}

async fn update_user(State(state): State<AppState>, Path(id): Path<i32>, Json(payload): Json<UserUpdate>) -> impl IntoResponse {
    let (tx, rx) = oneshot::channel();
    let cmd = CommandType::UserCommand(
        Arc::new(
            users::UserCommand::Update(
                UserUpdate {
                    id,
                    username: payload.username,
                    password_hash: payload.password_hash,
                    role_id: payload.role_id,
                    is_active: payload.is_active
                }
            )
        )
    );
    let msg = MainMsg::Command(Command { cmd, request_channel: tx });
    if let Err(response) = check_send_message(&state.from_api, msg).await {
        return response.into_response();
    }
    let res = rx.await;
    match res {
        Ok(Ok(())) => (StatusCode::OK, "User updated").into_response(),
        Ok(Err(err_msg)) => (StatusCode::INTERNAL_SERVER_ERROR, err_msg).into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response()
    }
}

async fn delete_user(State(state): State<AppState>, Path(id): Path<i32>) -> impl IntoResponse {
    let (tx, rx) = oneshot::channel();
    let cmd = CommandType::UserCommand(
        Arc::new(
            users::UserCommand::Delete(
                UserDelete{
                    id
                }
            )
        )
    );
    let msg = MainMsg::Command(Command { cmd, request_channel: tx });
    if let Err(response) = check_send_message(&state.from_api, msg).await {
        return response.into_response();
    }
    let res = rx.await;
    match res {
        Ok(Ok(())) => (StatusCode::OK, "User deleted").into_response(),
        Ok(Err(err_msg)) => (StatusCode::INTERNAL_SERVER_ERROR, err_msg).into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response()
    }
}
