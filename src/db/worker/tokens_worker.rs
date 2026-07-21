use crate::logger::printers;
use sqlx::{MySql, Pool};
use crate::db::schemas::tokens::RefreshToken;
use crate::db::worker::gen_fn::run_db_with_timeout;
use crate::messages::commands::command::{Command, CommandType};
use crate::messages::requests::tokens_request::TokensRequest;

pub fn get_token(pool: &Pool<MySql>, request: TokensRequest) {
    let pool = pool.clone();
    tokio::spawn(async move {
        let tx = request.request_channel;
        run_db_with_timeout(find_token(&pool, &request.token), 3, tx, "Request::GetToken").await;
        /*
        let res = find_token(&pool, &request.token).await;
        if tx.send(res).is_err(){
            printers::err("Помилка відправки калбеку Request::GetToken".to_string());
        }

         */
    });
}

pub fn update_token(pool: &Pool<MySql>, command: Command) {
    let pool = pool.clone();
    if let CommandType::TokenUpdate(refresh_token_update) = command.cmd {
        tokio::spawn(async move {
            let tx = command.request_channel;
            run_db_with_timeout(write_token(&pool, &refresh_token_update), 5, tx, "CommandType::TokenUpdate").await;
            /*
            let res = write_token(&pool, &refresh_token_update).await;
            if tx.send(res).is_err() {
                printers::err("Помилка відправки калбеку CommandType::TokenUpdate".to_string())
            }

             */
        });
    }
}

async fn find_token(pool: &Pool<MySql>, token: &str) -> Result<Option<RefreshToken>, ()> {
    let tk = sqlx::query_as::<_, RefreshToken>("SELECT user_id, user_role_id,token_hash, created_at, expires_at FROM refresh_tokens WHERE token_hash = ?")
        .bind(token)
        .fetch_optional(pool)
        .await.map_err(|e|{
        printers::err(format!("Помилка читання бази даних: {}", e));
    })?;
    Ok(tk)
}

async fn write_token(pool: &Pool<MySql>, refresh_token: &RefreshToken) -> Result<(), String> {
    sqlx::query("INSERT INTO refresh_tokens (
                user_id,
                user_role_id,
                token_hash,
                created_at,
                expires_at
                )
                VALUES (?, ?, ?, ?, ?)
                ON DUPLICATE KEY UPDATE
                user_role_id = VALUES(user_role_id),
                token_hash = VALUES(token_hash),
                created_at = VALUES(created_at),
                expires_at = VALUES(expires_at);")
        .bind(refresh_token.user_id)
        .bind(refresh_token.user_role_id)
        .bind(&refresh_token.token_hash)
        .bind(refresh_token.created_at)
        .bind(refresh_token.expires_at)
        .execute(pool).await.map_err(|e| {
        let msg = format!("Помилка збереження токену: {}", e);
        printers::err(msg.clone());
        msg
    })?;
    Ok(())
}