use std::sync::Arc;
use crate::api::init_axum::AppState;
use axum::{extract::State, Json, extract::Path, Router, middleware};
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
use crate::messages::requests::sub_group_request::{SubGroupsRequest, GetById, GetAll, GetByGroupId, GetAssigns};
use crate::messages::main_msg::MainMsg;
use crate::db::schemas::user_subgroups::{
    UserSubGroupCreate,
    UserSubGroupUpdate,
    UserSubGroupDelete,
};
use crate::messages::commands::sub_groups::SubGroupCommand;

pub fn user_subgroup_router() -> Router<AppState> {
    Router::new()
        .route("/sub_groups/create", post(create_sub_group))
        .route("/sub_groups/update/{id}", put(update_sub_group))
        .route("/sub_groups/delete/{id}", delete(delete_sub_group))
        .route("/sub_groups/get_all", get(get_all_sub_groups))
        .route("/sub_groups/get_by_id/{id}", get(get_sub_group_by_id))
        .route("/sub_groups/get_by_group_id/{group_id}", get(get_sub_groups_by_group_id))
        .route_layer(middleware::from_fn(admin_middleware))
        
        .route("/sub_groups/ui/assigns/{group_id}", get(ui_get_subgroups_assign))
}

async fn get_all_sub_groups(State(state): State<AppState>) -> impl IntoResponse {
    let (tx, rx) = oneshot::channel();
    let request = Request::GetSubGroup(
        SubGroupsRequest::GetAll(GetAll { request_channel: tx })
    );
    let main_msg = MainMsg::Request(request);
    if let Err(response) = check_send_message(&state.from_api, main_msg).await {
        return response.into_response();
    }
    match rx.await {
        Ok(Ok(sub_groups)) => (StatusCode::OK, Json(sub_groups)).into_response(),
        Ok(Err(_)) => (StatusCode::INTERNAL_SERVER_ERROR, "Error from database").into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response(),
    }
}

async fn get_sub_group_by_id(State(state): State<AppState>, Path(id): Path<i32>) -> impl IntoResponse {
    let (tx, rx) = oneshot::channel();
    let request = Request::GetSubGroup(
        SubGroupsRequest::GetById(GetById { id, request_channel: tx })
    );
    let main_msg = MainMsg::Request(request);
    if let Err(response) = check_send_message(&state.from_api, main_msg).await {
        return response.into_response();
    }
    match rx.await {
        Ok(Ok(sub_group)) => {
            if let Some(sub_group) = sub_group {
                (StatusCode::OK, Json(sub_group)).into_response()
            } else {
                (StatusCode::NOT_FOUND, "Subgroup not found").into_response()
            }
        },
        Ok(Err(_)) => (StatusCode::INTERNAL_SERVER_ERROR, "Error from database").into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response(),
    }
}

async fn get_sub_groups_by_group_id(State(state): State<AppState>, Path(group_id): Path<i32>) -> impl IntoResponse {
    let (tx, rx) = oneshot::channel();
    let request = Request::GetSubGroup(
        SubGroupsRequest::GetByGroupId(GetByGroupId { group_id, request_channel: tx })
    );
    let main_msg = MainMsg::Request(request);
    if let Err(response) = check_send_message(&state.from_api, main_msg).await {
        return response.into_response();
    }
    match rx.await {
        Ok(Ok(sub_groups)) => (StatusCode::OK, Json(sub_groups)).into_response(),
        Ok(Err(_)) => (StatusCode::INTERNAL_SERVER_ERROR, "Error from database").into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response(),
    }
}

async fn create_sub_group(State(state): State<AppState>, Json(payload): Json<UserSubGroupCreate>) -> impl IntoResponse {
    let (tx, rx) = oneshot::channel();
    let update_cmd = CommandType::SubGroupCommand(
        Arc::new(
            SubGroupCommand::Create(
                UserSubGroupCreate {
                    group_id: payload.group_id,
                    subgroup_name: payload.subgroup_name,
                    description: payload.description,
                }
            )
        )
    );
    let main_msg = MainMsg::Command(Command { cmd: update_cmd, request_channel: tx });
    if let Err(response) = check_send_message(&state.from_api, main_msg).await {
        return response.into_response();
    }
    match rx.await {
        Ok(Ok(_)) => (StatusCode::OK, "Subgroup created successfully").into_response(),
        Ok(Err(e)) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Error creating subgroup: {}", e)).into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response(),
    }
}

async fn update_sub_group(State(state): State<AppState>, Path(_id): Path<i32>, Json(payload): Json<UserSubGroupUpdate>) -> impl IntoResponse {
    let (tx, rx) = oneshot::channel();
    let update_cmd = CommandType::SubGroupCommand(
        Arc::new(
            SubGroupCommand::Update(
                UserSubGroupUpdate {
                    id: payload.id,
                    group_id: payload.group_id,
                    subgroup_name: payload.subgroup_name,
                    description: payload.description,
                }
            )
        )
    );
    let main_msg = MainMsg::Command(Command { cmd: update_cmd, request_channel: tx });
    if let Err(response) = check_send_message(&state.from_api, main_msg).await {
        return response.into_response();
    }
    match rx.await {
        Ok(Ok(_)) => (StatusCode::OK, "Subgroup updated successfully").into_response(),
        Ok(Err(e)) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Error updating subgroup: {}", e)).into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response(),
    }
}

async fn delete_sub_group(State(state): State<AppState>, Path(id): Path<i32>) -> impl IntoResponse {
    let (tx, rx) = oneshot::channel();
    let update_cmd = CommandType::SubGroupCommand(
        Arc::new(
            SubGroupCommand::Delete(
                UserSubGroupDelete { id }
            )
        )
    );
    let main_msg = MainMsg::Command(Command { cmd: update_cmd, request_channel: tx });
    if let Err(response) = check_send_message(&state.from_api, main_msg).await {
        return response.into_response();
    }
    match rx.await {
        Ok(Ok(_)) => (StatusCode::OK, "Subgroup deleted successfully").into_response(),
        Ok(Err(e)) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Error deleting subgroup: {}", e)).into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response(),
    }
}

async fn ui_get_subgroups_assign(State(state): State<AppState>, Path(group_id): Path<String>) -> impl IntoResponse {

    let res = match group_id.split(',').map(|id| id.trim().parse::<i32>()).collect::<Result<Vec<i32>, _>>() {
        Ok(ids) => {ids},
        Err(e) => {
            eprintln!("Error parsing ids: {}", e);
            return (StatusCode::BAD_REQUEST, "Invalid ids").into_response();
        }
    };

    let (tx, rx) = oneshot::channel();
    let request = Request::GetSubGroup(
        SubGroupsRequest::GetAssign(
            GetAssigns { groups_ids: res, request_channel: tx }
        )
    );

    let msg = MainMsg::Request(request);
    if let Err(response) = check_send_message(&state.from_api, msg).await {
        return response.into_response();
    }

    match rx.await {
        Ok(Ok(user_ids)) => (StatusCode::OK, Json(user_ids)).into_response(),
        Ok(Err(_)) => (StatusCode::INTERNAL_SERVER_ERROR, "Error from database").into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response(),
    }
}