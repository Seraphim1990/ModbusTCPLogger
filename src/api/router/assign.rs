use std::sync::Arc;
use crate::api::init_axum::AppState;
use axum::{extract::State, Json, extract::Path, Router, middleware};
use axum::response::IntoResponse;
use tokio::sync::oneshot;
use axum::http::StatusCode;
use axum::routing::{delete, post};
use crate::api::router::handle_get_request::check_send_message;
use crate::api::router::middlewares::admin_middleware;
use crate::messages::commands::command::{
    CommandType, Command
};
use crate::messages::commands::asign::*;
use crate::messages::main_msg::MainMsg;

pub fn assign_router() -> Router<AppState> {
    Router::new()
        .route("/assign/group", post(create_assign_group))
        .route("/assign/values", post(create_assign_values))
        .route("/assign/clean/users/{user_id}", delete(clean_group_assignments))
        .route("/assign/clean/subgroups/{subgroup_id}", delete(clean_values_assignments))
        .route_layer(middleware::from_fn(admin_middleware))
}

async fn create_assign_group(State(state): State<AppState>, Json(payload): Json<AssignGroupsCommand>) -> impl IntoResponse {
    let (tx, rx) = oneshot::channel();
    let assign_cmd = CommandType::AssignGroupsAndValuesCommand(
        Arc::new(
            AssignGroupsAndValuesCommand::Groups(AssignGroupsCommand {
                user_id: payload.user_id,
                group_ids: payload.group_ids,
            })
        )
    );
    let main_msg = MainMsg::Command(Command { cmd: assign_cmd, request_channel: tx });
    if let Err(response) = check_send_message(&state.from_api, main_msg).await {
        return response.into_response();
    }
    match rx.await {
        Ok(Ok(_)) => (StatusCode::OK, "Groups assigned successfully").into_response(),
        Ok(Err(e)) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Error assigning groups: {}", e)).into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response(),
    }
}

async fn create_assign_values(State(state): State<AppState>, Json(payload): Json<AssignValuesCommand>) -> impl IntoResponse {
    let (tx, rx) = oneshot::channel();
    let assign_cmd = CommandType::AssignGroupsAndValuesCommand(
        Arc::new(
            AssignGroupsAndValuesCommand::Values(
                AssignValuesCommand{
                    subgroup_id: payload.subgroup_id,
                    value_unit_ids: payload.value_unit_ids,
                }
            )
        )
    );
    let main_msg = MainMsg::Command(Command { cmd: assign_cmd, request_channel: tx });
    if let Err(response) = check_send_message(&state.from_api, main_msg).await {
        return response.into_response();
    }
    match rx.await {
        Ok(Ok(_)) => (StatusCode::OK, "Values assigned successfully").into_response(),
        Ok(Err(e)) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Error assigning values: {}", e)).into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response(),
    }
}

async fn clean_group_assignments(State(state): State<AppState>, Path(user_id): Path<i32>) -> impl IntoResponse {
    let (tx, rx) = oneshot::channel();
    let assign_cmd = CommandType::AssignGroupsAndValuesCommand(
        Arc::new(
            AssignGroupsAndValuesCommand::Groups(AssignGroupsCommand {
                user_id,
                group_ids: Vec::new(),
            })
        )
    );
    let main_msg = MainMsg::Command(Command { cmd: assign_cmd, request_channel: tx });
    if let Err(response) = check_send_message(&state.from_api, main_msg).await {
        return response.into_response();
    }
    match rx.await {
        Ok(Ok(_)) => (StatusCode::OK, "Group assignments cleaned successfully").into_response(),
        Ok(Err(e)) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Error cleaning group assignments: {}", e)).into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response(),
    }
}

async fn clean_values_assignments(State(state): State<AppState>, Path(subgroup_id): Path<i32>) -> impl IntoResponse {
    let (tx, rx) = oneshot::channel();
    let assign_cmd = CommandType::AssignGroupsAndValuesCommand(
        Arc::new(
            AssignGroupsAndValuesCommand::Values(AssignValuesCommand{
                subgroup_id,
                value_unit_ids: Vec::new(),
            })
        )
    );
    let main_msg = MainMsg::Command(Command { cmd: assign_cmd, request_channel: tx });
    if let Err(response) = check_send_message(&state.from_api, main_msg).await {
        return response.into_response();
    }
    match rx.await {
        Ok(Ok(_)) => (StatusCode::OK, "Values assignments cleaned successfully").into_response(),
        Ok(Err(e)) => (StatusCode::INTERNAL_SERVER_ERROR, format!("Error cleaning values assignments: {}", e)).into_response(),
        Err(_) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response(),
    }
}