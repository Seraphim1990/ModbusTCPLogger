use std::ops::Deref;
use sqlx::{Error, MySql, Pool};
use crate::messages::requests::user_request::*;
use crate::messages::commands::{
    command::{Command, CommandType},
    users::UserCommand
};
use crate::logger::printers;
use sqlx::mysql::MySqlQueryResult;
use crate::db::schemas::users::{LoginRequest, UserCreate, UserRead, UserUpdate};
use crate::db::worker::gen_fn::run_db_with_timeout;

pub fn user_command(pool: &Pool<MySql>, command: Command){
    let pool = pool.clone();
    if let CommandType::UserCommand(user) = command.cmd {
        let tx = command.request_channel;
        match user.as_ref() {
            UserCommand::Create(_) => {
                let user = user.clone();
                tokio::spawn(async move {
                    if let UserCommand::Create(user) = user.deref() {
                        run_db_with_timeout(create_user(&pool, user), 5, tx, "UserCommand::Create").await;
                        /*
                        let res = create_user(&pool, user).await;
                        if tx.send(res).is_err() {
                            printers::err("Помилка відправки калбеку UserCommand::Create".to_string())
                        }

                         */
                    };
                });
            },
            UserCommand::Update(_) => {
                let user = user.clone();
                tokio::spawn(async move {
                    if let UserCommand::Update(user) = user.deref() {
                        run_db_with_timeout(update_user(&pool, user), 5, tx, "UserCommand::Update").await;
                        /*
                        let res = update_user(&pool, user).await;
                        if tx.send(res).is_err() {
                            printers::err("Помилка відправки калбеку UserCommand::Update".to_string())
                        }

                         */
                    }
                });
            },
            UserCommand::Delete(_) => {
                let user = user.clone();
                tokio::spawn(async move {
                   if let UserCommand::Delete(user) = user.deref() {
                       run_db_with_timeout(delete_user(&pool, user.id), 5, tx, "UserCommand::Delete").await;
                       /*
                       let res = delete_user(&pool, user.id).await;
                       if tx.send(res).is_err() {
                           printers::err("Помилка відправки калбеку UserCommand::Delete".to_string())
                       }

                        */
                   }
                });
            },
        }
    }
}

pub fn users_get(pool: &Pool<MySql>, request: UserRequest){
    let pool = pool.clone();

    match request {
        UserRequest::GetById(request) => {
            tokio::spawn(async move {
                let tx = request.request_channel;
                run_db_with_timeout(get_user_by_id(&pool, request.id), 3, tx, "UserRequest::GetById").await;
                /*
                let res = get_user_by_id(&pool, request.id).await;
                if tx.send(res).is_err() {
                    printers::err(format!("Помилка відправлення відповіді UserRequest::GetById: {}", request.id));
                }

                 */
            });
        },
        UserRequest::GetAll(request) => {
            tokio::spawn(async move {
                let tx = request.request_channel;
                run_db_with_timeout(get_all_users(&pool), 3, tx, "UserRequest::GetAll").await;
                /*
                let res = get_all_users(&pool).await;
                if tx.send(res).is_err() {
                    printers::err("Помилка відправки відповіді UserRequest::GetAll".to_string());
                }

                 */
            });
        }
        UserRequest::Verify(request) => {
            tokio::spawn(async move {
                let tx = request.request_channel;
                run_db_with_timeout(verify_user_credentials(&pool, &request.user), 3, tx, "UserRequest::Verify").await;
                /*
                let res = verify_user_credentials(&pool, &request.user).await;
                if tx.send(res).is_err() {
                    printers::err("Помилка відправки відповіді UserRequest::GetExist".to_string());
                }

                 */
            });
        }
    }
}

async fn get_user_by_id(pool: &Pool<MySql>, id: i32) -> Result<Option<UserRead>, ()> {

    let user = sqlx::query_as::<_, UserRead>("SELECT id, username, password_hash, role_id, is_active FROM users WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            printers::err(format!("Помилка читання користувача з бази даних: {e}"));
        })?;

    Ok(user)
}

async fn get_all_users(pool: &Pool<MySql>) -> Result<Vec<UserRead>, ()> {
    let users = sqlx::query_as::<_, UserRead>("SELECT id, username, password_hash, role_id, is_active FROM users")
        .fetch_all(pool)
        .await
        .map_err(|e|{
            printers::err(format!("Помилка читання всих користувачів з бази даних: {e}"));
        })?;
    Ok(users)
}

async fn verify_user_credentials(pool: &Pool<MySql>, credentials: &LoginRequest) -> Result<Option<UserRead>, ()> {
    // 1. Шукаємо користувача ТІЛЬКИ за іменем
    let user_opt = sqlx::query_as::<_, UserRead>(
        "SELECT id, username, password_hash, role_id, is_active FROM users WHERE username = ?"
    )
        .bind(&credentials.username)
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            printers::err(format!("Помилка бази даних при авторизації: {e}"));
        })?;
    Ok(user_opt)
}

async fn create_user(pool: &Pool<MySql>, user: &UserCreate) -> Result<(), String> {

    found_role_id(pool, user.role_id).await?;

    let insert_res = sqlx::query("INSERT INTO users (username, password_hash, role_id, is_active) VALUES (?, ?, ?, ?)")
        .bind(&user.username)
        .bind(&user.password_hash)
        .bind(user.role_id)
        .bind(true)
        .execute(pool).await;

    check_db_write_user_err(insert_res)
}
async fn update_user(pool: &Pool<MySql>, user: &UserUpdate) -> Result<(), String> {
    if let Some(role_id) = user.role_id {
        found_role_id(pool, role_id).await?;
    }

    let update_res = sqlx::query("UPDATE users SET
                 username = COALESCE(?, username),
                 password_hash = COALESCE(?, password_hash),
                 role_id = COALESCE(?, role_id),
                 is_active = ?
             WHERE id = ?")
        .bind(&user.username)
        .bind(&user.password_hash)
        .bind(user.role_id)
        .bind(user.is_active.unwrap_or(true))
        .bind(user.id)
        .execute(pool).await;

    check_db_write_user_err(update_res)
}

async fn delete_user(pool: &Pool<MySql>, id: i32) -> Result<(), String> {
    sqlx::query("DELETE FROM users WHERE id = ?")
        .bind(id)
        .execute(pool).await
        .map_err(|e|{
            let msg = format!("Помилка видалення користувача: {}", e);
            printers::err(msg.clone());
            msg
        })?;
    Ok(())
}

fn check_db_write_user_err(db_err: Result<MySqlQueryResult, Error>) -> Result<(), String> {
    if let Err(Error::Database(db_err)) = &db_err { // 1062 — це код MySQL для Duplicate Entry
        if db_err.code().as_deref() == Some("1062") {
            return Err("Користувач з таким іменем уже існує".to_string());
        }
    }
    db_err.map_err(|e| {
        let msg = format!("Помилка запису в БД: {e}");
        printers::err(msg.clone());
        msg
    })?;     // Якщо інша помилка бази — прокидуємо далі
    Ok(())
}

async fn found_role_id(pool: &Pool<MySql>, role_id: i32) -> Result<(), String> {
    sqlx::query("SELECT id FROM roles WHERE id = ?")
        .bind(role_id)
        .fetch_optional(pool).await
        .map_err(|e|{
            let msg = format!("Помилка читання з бази даних: {e}");
            printers::err(msg.clone());
            msg
        })?.ok_or_else(|| format!("Вказаної ролі (role_id: {}) не існує", role_id))?;
    Ok(())
}