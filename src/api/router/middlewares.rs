use axum::{
    extract::Request,
    http::{header, StatusCode},
    middleware::Next,
    response::Response,
};
use jsonwebtoken::{decode, DecodingKey, Validation};
use crate::api::init_axum::{Claims, JWT_KEY};

pub async fn admin_middleware(request: Request,
                              next: Next,
) -> Result<Response, StatusCode> {

    let claims = request
        .extensions()
        .get::<Claims>()
        .ok_or(StatusCode::UNAUTHORIZED)?;

    if claims.role_id != 1 {
        return Err(StatusCode::FORBIDDEN);
    }

    Ok(next.run(request).await)
}

pub async fn auth_middleware(mut request: Request,
                              next: Next,
) -> Result<Response, StatusCode> {
    let auth_header = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .ok_or(StatusCode::UNAUTHORIZED)?;
    // 2. Перевіряємо, чи він починається з "Bearer "
    if !auth_header.starts_with("Bearer ") {
        return Err(StatusCode::BAD_REQUEST);
    }

    let token = &auth_header[7..]; // Відрізаємо "Bearer "

    // 3. Декодуємо та валідуємо JWT токен
    let token_data = decode::<Claims>(
        token,
        &DecodingKey::from_secret(JWT_KEY),
        &Validation::default(), // Перевіряє exp і iat автоматично
    ).map_err(|_| StatusCode::UNAUTHORIZED)?;

    request
        .extensions_mut()
        .insert(token_data.claims);
    Ok(next.run(request).await)
}