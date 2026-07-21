use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use axum::extract::State;
use axum::{Json, Router};
use axum::response::IntoResponse;
use axum::routing::{get, post};
use serde::{Deserialize, Serialize};
use crate::api::init_axum::AppState;
use crate::logger::printers;
use crate::api::init_axum::{Claims, JWT_KEY};
use tokio::sync::{oneshot};
use crate::messages::main_msg::MainMsg;
use crate::db::schemas::users::LoginRequest;
use crate::messages::requests::user_request::{GetVerifyRequest, UserGetById, UserRequest};
use crate::messages::requests::request_struct::Request;
use crate::api::router::users::UserSendRequest;
use crate::api::router::handle_get_request::check_send_message;
use axum::http::{HeaderMap, StatusCode};
use jsonwebtoken::{decode, DecodingKey, Validation};
use jsonwebtoken::{encode, Header, EncodingKey};
use uuid::Uuid;
use crate::db::schemas::tokens::RefreshToken;
use crate::messages::commands::command::{Command, CommandType};
use crate::messages::requests::tokens_request::TokensRequest;

/*
#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: i32, // ID користувача (subject)
    pub exp: i64,    // Час, коли токен помре (Unix timestamp)
    pub iat: i64,    // Час створення токена (Issued at)
}
 */

pub fn auth() -> Router<AppState> {
    Router::new()
        .route("/auth/login", post(login))
        .route("/auth/logout", post(logout))
        .route("/auth/refresh_token", post(refresh_token))
        .route("/auth/me", get(me))
        .route("/bit", get(bit))
}
async fn bit() -> impl IntoResponse {
    (StatusCode::OK, "Hello world")
}

#[derive(Debug, Deserialize, Serialize)]
struct UserLogin{
    username: String,
    password: String,
}

#[derive(Serialize)]
struct AuthResponse {
    access_token: String,
    refresh_token: String,
    token_type: String,
}

async fn login(State(state): State<AppState>, Json(payload): Json<UserLogin>) -> impl IntoResponse {
    let (tx, rx) = oneshot::channel();
    let user = UserRequest::Verify(
        GetVerifyRequest{
            request_channel: tx,
            user: LoginRequest{
                password_raw: payload.password.clone(),
                username: payload.username.clone(),
            }
        }
    );
    let msg = MainMsg::Request(
        Request::GetUser(user)
    );

    if let Err(e) = check_send_message(&state.from_api, msg).await {
        return (StatusCode::INTERNAL_SERVER_ERROR, e).into_response();
    }
    let user = match rx.await {
        Ok(Ok(user)) => {
            match user {
                Some(user) => user,
                None => {
                    printers::err(format!("Користувача {} не знайдено", payload.username));
                    return (StatusCode::UNAUTHORIZED, "User not found").into_response();
                }
            }
        },
        Ok(Err(_)) | Err(_)=> {
            return (StatusCode::INTERNAL_SERVER_ERROR, "Server error").into_response()
        }
    };

    if user.password_hash != payload.password {
        return (StatusCode::UNAUTHORIZED, "Password incorrect").into_response();
    }

    let (access_token, curr_time) = match generate_access_token(user.id, user.role_id) {
        Ok(res) => res,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Error generating access token").into_response(),
    };

    let refresh_token = Uuid::new_v4().simple().to_string();

    let (tx, rx) = oneshot::channel();

    let cmd = CommandType::TokenUpdate(
        Arc::new(
            RefreshToken{
                user_id: user.id ,
                user_role_id: user.role_id,
                token_hash: refresh_token.clone(),
                created_at: curr_time,
                expires_at: curr_time + 2678400  //  Додаємо місяць в секундах
            }
        )
    );

    let ref_token_cmd = MainMsg::Command(
        Command{
            request_channel: tx,
            cmd
        }
    );

    if let Err(e) = check_send_message(&state.from_api, ref_token_cmd).await {
        return e.into_response();
    }

    if rx.await.is_err() {
        return (StatusCode::INTERNAL_SERVER_ERROR, "Error generating access token").into_response()
    }

    let response_body = AuthResponse {
        access_token,
        refresh_token,
        token_type: "Bearer".to_string(),
    };

    printers::event(format!("Користувач {} логін", user.username));

    (StatusCode::OK, Json(response_body)).into_response()
}

async fn logout(state: State<AppState>, Json(payload): Json<RefreshRequest>) -> impl IntoResponse {
    let (tx, rx) = oneshot::channel();
    let req = Request::GetToken(
        TokensRequest{
            token: payload.refresh_token.clone(),
            request_channel: tx
        }
    );
    let msg = MainMsg::Request(req);
    if let Err(e) = check_send_message(&state.from_api, msg).await {
        return e.into_response();
    };
    let token_record = match rx.await {
        Ok(Ok(Some(token))) => token,
        Ok(Ok(None)) => return (StatusCode::UNAUTHORIZED, "Invalid refresh token").into_response(),
        _ => return (StatusCode::INTERNAL_SERVER_ERROR, "Server error").into_response(),
    };

    let current_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs() as i64;

    let (tx, rx) = oneshot::channel();

    let cmd = CommandType::TokenUpdate(
        Arc::new(
            RefreshToken{
                user_id: token_record.user_id ,
                user_role_id: token_record.user_role_id,
                token_hash: token_record.token_hash,
                created_at: current_time,
                expires_at: current_time // просто обнуляємо доступ
            }
        )
    );

    let ref_token_cmd = MainMsg::Command(
        Command{
            request_channel: tx,
            cmd
        }
    );

    if let Err(e) = check_send_message(&state.from_api, ref_token_cmd).await {
        return e.into_response();
    }

    if rx.await.is_err() {
        return (StatusCode::INTERNAL_SERVER_ERROR, "Error generating access token").into_response()
    }

    (StatusCode::OK, "Ok").into_response()
}

#[derive(Debug, Deserialize)]
struct RefreshRequest {
    refresh_token: String,
}

async fn refresh_token(state: State<AppState>, Json(payload): Json<RefreshRequest>) -> impl IntoResponse {
    let (tx, rx) = oneshot::channel();
    let req = Request::GetToken(
        TokensRequest{
            token: payload.refresh_token.clone(),
            request_channel: tx
        }
    );
    let msg = MainMsg::Request(req);
    if let Err(e) = check_send_message(&state.from_api, msg).await {
        return e.into_response();
    };
    let token_record = match rx.await {
        Ok(Ok(Some(token))) => token,
        Ok(Ok(None)) => return (StatusCode::UNAUTHORIZED, "Invalid refresh token").into_response(),
        _ => return (StatusCode::INTERNAL_SERVER_ERROR, "Server error").into_response(),
    };
    let current_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs() as i64;

    if token_record.expires_at < current_time {
        return (StatusCode::UNAUTHORIZED, "Refresh token expired. Please login again").into_response(); // видаляти не буду, при авторизації він перезапишеться для цього користувача
    }

    // 3. Токен валідний! ГЕНЕРУЄМО НОВИЙ ACCESS TOKEN
    let (access_token, _) = match generate_access_token(token_record.user_id, token_record.user_role_id) {
        Ok(res) => res,
        Err(_) => return (StatusCode::INTERNAL_SERVER_ERROR, "Error generating access token").into_response(),
    };

    // 4. Повертаємо оновлений access_token.
    // Старий refresh_token повертаємо назад (або можеш згенерувати новий, якщо хочеш схему single-use)
    let response_body = AuthResponse {
        access_token,
        refresh_token: payload.refresh_token,
        token_type: "Bearer".to_string(),
    };

    (StatusCode::OK, Json(response_body)).into_response()
}

async fn me(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {  // -> UserSendRequest
    // 1. Шукаємо заголовок Authorization
    let auth_header = match headers.get("Authorization").and_then(|h| h.to_str().ok()) {
        Some(header) => header,
        None => return (StatusCode::UNAUTHORIZED, "Missing Authorization Header").into_response(),
    };
    // 2. Перевіряємо, чи він починається з "Bearer "
    if !auth_header.starts_with("Bearer ") {
        return (StatusCode::BAD_REQUEST, "Invalid Authorization format").into_response();
    }

    let token = &auth_header[7..]; // Відрізаємо "Bearer "

    // 3. Декодуємо та валідуємо JWT токен
    let token_data = match decode::<Claims>(
        token,
        &DecodingKey::from_secret(JWT_KEY),
        &Validation::default(), // Перевіряє exp і iat автоматично
    ) {
        Ok(data) => data,
        Err(_) => return (StatusCode::UNAUTHORIZED, "Invalid or expired token").into_response(),
    };

    let current_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs() as i64;

    if token_data.claims.exp < current_time {
        return (StatusCode::UNAUTHORIZED, "Token expired. Please login again").into_response();
    }

    let user_id = token_data.claims.id;

    let (tx, rx) = oneshot::channel();
    let req = Request::GetUser(
        UserRequest::GetById(
            UserGetById {
                id: user_id,
                request_channel: tx
            }
        )
    );
    let msg = MainMsg::Request(req);
    if let Err(e) = check_send_message(&state.from_api, msg).await {
        return e.into_response();
    }
    match rx.await {
        Ok(Ok(user)) => {
            match user {
                Some(user) => {
                    let us = UserSendRequest { id: user.id, name: user.username, role_id: user.role_id, };
                    (StatusCode::OK, Json(us)).into_response()
                }
                None => (StatusCode::NOT_FOUND, "User not found").into_response(),  // по харошому тут би уїбать паніку, але всяке трапляється, нехай перезаходить
            }
        }
        Ok(Err(_)) | Err(_) => {
            (StatusCode::INTERNAL_SERVER_ERROR, "Server error").into_response()
        }
    }
}

fn generate_access_token(id: i32, role_id: i32) -> Result<(String, i64), ()> {
    let current_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs() as i64;

    let expiration = current_time + 900; // 15 хвилин

    let claims = Claims {
        id,
        role_id,
        exp: expiration,
        iat: current_time
    };

    match encode(
        &Header::default(),
        &claims,
        &EncodingKey::from_secret(JWT_KEY)
    ) {
        Ok(t) => Ok((t, current_time)),
        Err(_) => Err(()),
    }
}