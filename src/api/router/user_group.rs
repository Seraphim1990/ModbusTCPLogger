use std::sync::Arc;
use crate::api::init_axum::{AppState, Claims};
use axum::{extract::State, Json, extract::Path, Router, middleware, Extension};
use axum::response::IntoResponse;
use tokio::sync::oneshot;
use axum::http::StatusCode;
use axum::routing::{delete, get, post, put};
use crate::api::router::handle_get_request::check_send_message;
use crate::api::router::middlewares::admin_middleware;
use crate::messages::commands::command::{
    CommandType, Command
};
use crate::messages::requests::request_struct::Request;
use crate::messages::requests::group_request::{GroupRequest, GroupGetByGroupId, GroupGetAll, GetByUserId, UiUserGroupsRequest};
use crate::messages::main_msg::MainMsg;
use crate::db::schemas::user_groups::*;
use crate::messages::commands::groups::GroupCommand;

pub fn user_group_router() -> Router<AppState> {
    Router::new()
        .route("/user_group/get_by_user_id/{id}", get(get_user_groups))
        .route("/user_group/create", post(create_group))
        .route("/user_group/update/{id}", put(update_group))
        .route("/user_group/delete/{id}", delete(delete_group))
        .route("/user_group/get_by_id/{id}", get(get_group_by_id))
        .route_layer(middleware::from_fn(admin_middleware))

        .route("/user_group/all", get(get_all_groups))
        .route("/user_group/ui/{group_id}", get(ui_get_by_group_id))
        .route("/user_group/me", get(me))
}

async fn get_user_groups(
    State(state): State<AppState>,
    Path(user_id): Path<i32>,
) -> impl IntoResponse {
    let (tx, rx) = oneshot::channel();
    let request = Request::GetGroup(
        GroupRequest::GetByUserId(GetByUserId { user_id, request_channel: tx })
    );
    let main_msg = MainMsg::Request(request);
    if let Err(response) = check_send_message(&state.from_api, main_msg).await {
        return response.into_response();
    }
    match rx.await {
        Ok(Ok(groups)) => (StatusCode::OK, Json(groups)).into_response(),
        Ok(Err(_)) => (StatusCode::INTERNAL_SERVER_ERROR, "Error from database").into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response(),
    }
}

async fn get_all_groups(State(state): State<AppState>) -> impl IntoResponse {
    let (tx, rx) = oneshot::channel();
    let request = Request::GetGroup(
        GroupRequest::GetAll(GroupGetAll { request_channel: tx })
    );
    let main_msg = MainMsg::Request(request);
    if let Err(response) = check_send_message(&state.from_api, main_msg).await {
        return response.into_response();
    }
    match rx.await {
        Ok(Ok(groups)) => (StatusCode::OK, Json(groups)).into_response(),
        Ok(Err(_)) => (StatusCode::INTERNAL_SERVER_ERROR, "Error from database").into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response(),
    }
}

async fn get_group_by_id(State(state): State<AppState>, Path(group_id): Path<i32>) -> impl IntoResponse {
    let (tx, rx) = oneshot::channel();
    let request = Request::GetGroup(
        GroupRequest::GetById(
            GroupGetByGroupId { group_id, request_channel: tx }
        )
    );
    let main_msg = MainMsg::Request(request);
    if let Err(response) = check_send_message(&state.from_api, main_msg).await {
        return response.into_response();
    }
    match rx.await {
        Ok(Ok(group)) => {
            if let Some(group) = group {
                (StatusCode::OK, Json(group)).into_response()
            } else {
                (StatusCode::NOT_FOUND, "Group not found").into_response()
            }
        },
        Ok(Err(_)) => (StatusCode::INTERNAL_SERVER_ERROR, "Error from database").into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response(),
    }
}

async fn create_group(State(state): State<AppState>, Json(payload): Json<UserGroupCreate>) -> impl IntoResponse {
    let (tx, rx) = oneshot::channel();
    let create_cmd = CommandType::GroupCommand(
        Arc::new(
            GroupCommand::Create(
                payload
            )
        )
    );
    let main_msg = MainMsg::Command(Command { cmd: create_cmd, request_channel: tx });
    if let Err(response) = check_send_message(&state.from_api, main_msg).await {
        return response.into_response();
    }
    match rx.await {
        Ok(Ok(_)) => (StatusCode::CREATED, "Group created successfully").into_response(),
        Ok(Err(e)) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Error creating group: {}", e)).into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response(),
    }
}

async fn update_group(State(state): State<AppState>, Path(_): Path<i32>, Json(payload): Json<UserGroupUpdate>) -> impl IntoResponse {
    let (tx, rx) = oneshot::channel();
    let update_cmd = CommandType::GroupCommand(
        Arc::new(
            GroupCommand::Update(payload)
        )
    );
    let main_msg = MainMsg::Command(Command { cmd: update_cmd, request_channel: tx });
    if let Err(response) = check_send_message(&state.from_api, main_msg).await {
        return response.into_response();
    }
    match rx.await {
        Ok(Ok(_)) => (StatusCode::OK, "Group updated successfully").into_response(),
        Ok(Err(e)) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Error updating group: {}", e)).into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response(),
    }
}

async fn delete_group(State(state): State<AppState>, Path(id): Path<i32>) -> impl IntoResponse {
    let (tx, rx) = oneshot::channel();
    let delete_cmd = CommandType::GroupCommand(
        Arc::new(
            GroupCommand::Delete(
                UserGroupDelete {id}
            )
        )
    );
    let main_msg = MainMsg::Command(Command { cmd: delete_cmd, request_channel: tx });
    if let Err(response) = check_send_message(&state.from_api, main_msg).await {
        return response.into_response();
    }
    match rx.await {
        Ok(Ok(_)) => (StatusCode::NO_CONTENT, "Group deleted successfully").into_response(),
        Ok(Err(e)) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Error deleting group: {}", e)).into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response(),
    }
}

// UI clock

async fn ui_get_by_group_id(State(state): State<AppState>, Path(group_id): Path<i32>) -> impl IntoResponse {
    let (tx, rx) = oneshot::channel();
    let request = Request::GetGroup(
        GroupRequest::GetForUi(
            UiUserGroupsRequest { id: group_id, request_channel: tx }
        )
    );
    let main_msg = MainMsg::Request(request);
    if let Err(response) = check_send_message(&state.from_api, main_msg).await {
        return response.into_response();
    }
    match rx.await {
        Ok(Ok(groups)) => {
            (StatusCode::OK, Json(groups)).into_response()
        },
        Ok(Err(_)) => (StatusCode::INTERNAL_SERVER_ERROR, "Error from database").into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response(),
    }
}

async fn me(State(state): State<AppState>,
            Extension(claims): Extension<Claims>,) -> impl IntoResponse {

    match claims.role_id {
        3 | 4 => {
            let user_id = claims.id;
            get_user_groups(State(state), Path(user_id)).await.into_response()
        }
        2 | 1 => {
            get_all_groups(State(state)).await.into_response()
        }
        _ => {
            (StatusCode::FORBIDDEN, "").into_response()
        }
    }
}