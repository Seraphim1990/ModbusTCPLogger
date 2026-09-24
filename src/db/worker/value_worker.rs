use std::sync::Arc;
use crate::messages::requests::value_request::*;
use crate::db::schemas::value_unit::{ValueCreate, ValueDelete, ValueRead, ValueUpdate};
use crate::logger::printers;
use sqlx::{FromRow, MySql, Pool};
use tokio::sync::mpsc;
use crate::db::schemas::node::NodeRead;
use crate::db::worker::gen_fn::run_db_with_timeout;
use crate::messages::commands::command::{Command, CommandType};
use crate::messages::commands::value::ValueCommand;
use crate::messages::config_event::{ConfigEvent, ConfigEventType};
use crate::reader::reader_loop::decoding_plugins::plugin_loader::check_json;

pub fn command_value(pool: &Pool<MySql>, command: Command, tx_to_reader: mpsc::Sender<ConfigEvent>) {
    let pool = pool.clone();
    if let CommandType::ValueCommand(value) = command.cmd {
        let tx = command.request_channel;
        match value.as_ref() {
            ValueCommand::Create(_) => {
                let value = value.clone();
                tokio::spawn(async move {
                    if let ValueCommand::Create(value) = value.as_ref() {
                        run_db_with_timeout(create_value(&pool, value, tx_to_reader), 5, tx, "ValueCommand::Create").await;
                        /*
                        let res = create_value(&pool, value, tx_to_reader).await;
                        if tx.send(res).is_err() {
                            let msg = "Помилка відправки калбеку для ValueCommand::Create".to_string();
                            printers::err(msg);
                        }

                         */
                    }
                });
            },
            ValueCommand::Delete(_) => {
                let value = value.clone();
                tokio::spawn(async move {
                   if let ValueCommand::Delete(value) = value.as_ref() {
                       run_db_with_timeout(delete_value(&pool, value, tx_to_reader), 5, tx, "ValueCommand::Delete").await;
                       /*
                       let res = delete_value(&pool, value, tx_to_reader).await;
                       if tx.send(res).is_err() {
                           let msg = format!("Помилка відправки калбеку для ValueCommand::Delete id: {}", value.id);
                           printers::err(msg);
                       }

                        */
                   }
                });
            },
            ValueCommand::Update(_) => {
                let value = value.clone();
                tokio::spawn(async move {
                    if let ValueCommand::Update(value) = value.as_ref() {
                        run_db_with_timeout(value_update(&pool, value, tx_to_reader), 5, tx, "ValueCommand::Update").await;
                        /*
                        let res = value_update(&pool, value, tx_to_reader).await;
                        if tx.send(res).is_err() {
                            let msg = format!("Помилка відправки калбеку для ValueCommand::Update id: {}", value.id);
                            printers::err(msg);
                        }

                         */
                    }
                });
            },
        }
    }
}

pub fn value_get(pool: &Pool<MySql>, request: ValueRequest) {
    let pool = pool.clone();
    match request {
        ValueRequest::GetValue(request) => {
            tokio::spawn(async move {
                let tx = request.request_channel;
                run_db_with_timeout(get_value_by_id(&pool, request.id), 3, tx, "ValueRequest::GetValue").await;
                /*
                let res = get_value_by_id(&pool, request.id).await;
                if tx.send(res).is_err() {
                    let msg = format!("Помилка відправки ValueRequest::GetValue {}", request.id);
                    printers::warn(msg);
                }

                 */
            });
        },
        ValueRequest::GetByDeviceId(request) => {
            tokio::spawn(async move {
                let tx = request.request_channel;
                run_db_with_timeout(get_value_by_device_id(&pool, request.device_id), 3, tx, "ValueRequest::GetByDeviceId").await;
                /*
                let res = get_value_by_device_id(&pool, request.device_id).await;
                if tx.send(res).is_err() {
                    let msg = format!("Помилка відправки ValueRequest::GetByDeviceId {}", request.device_id);
                    printers::err(msg);
                }

                 */
            });
        },
        ValueRequest::GetAll(request) => {
            tokio::spawn(async move {
                let tx = request.request_channel;
                run_db_with_timeout(get_all_values(&pool), 3, tx, "ValueRequest::GetAll").await;
                /*
                let res = get_all_values(&pool).await;
                if tx.send(res).is_err() {
                    printers::err(String::from("Помилка відправки ValueRequest::GetAll"));
                }

                 */
            });
        },
        ValueRequest::GetLoggingOnly(request) => {
            tokio::spawn(async move {
                let tx = request.request_channel;
                run_db_with_timeout(get_logging_only(&pool), 3, tx, "ValueRequest::GetLoggingOnly").await;
                /*
                let res = get_logging_only(&pool).await;
                if tx.send(res).is_err() {
                    printers::err(String::from("Помилка відправки ValueRequest::GetLoggingOnly"));
                }

                 */
            });
        }
    }
}

async fn get_value_by_id(pool: &Pool<MySql>, id: i32) -> Result<Option<ValueRead>, ()> {
    let value = sqlx::query_as::<_, ValueRead>(
        "SELECT id,
       parent_device_id,
       value_name,
       value_tag,
       description,
       decoding_type,
       settings,
       is_logging
        FROM value_units WHERE id = ?"
    )
        .bind(id)
        .fetch_optional(pool)
        .await;
    match value {
        Ok(value) => {
            if value.is_none() {
                Ok(None)
            } else {
                Ok(value)
            }
        }
        Err(e) => {
            let msg = format!("Помилка отримання значення від БД по id: {:?}", e);
            printers::err(msg);
            Err(())
        },
    }
}

pub async fn get_value_by_device_id(pool: &Pool<MySql>, id: i32) -> Result<Vec<ValueRead>, ()> {
    let values = sqlx::query_as::<_, ValueRead>(
        "SELECT id,
       parent_device_id,
       value_name,
       value_tag,
       description,
       decoding_type,
       settings,
       is_logging
        FROM value_units WHERE parent_device_id = ?"
    )
        .bind(id)
        .fetch_all(pool)
        .await;
    match values {
        Ok(values) => Ok(values),
        Err(e) => {
            let msg = format!("Помилка отримання значення від БД по device_id: {:?}", e);
            printers::err(msg);
            Err(())
        }
    }
}

async fn get_all_values(pool: &Pool<MySql>) -> Result<Vec<ValueRead>, ()> {
    let values = sqlx::query_as::<_, ValueRead>(
        "SELECT id,
       parent_device_id,
       value_name,
       value_tag,
       description,
       decoding_type,
       settings,
       is_logging
        FROM value_units"
    )
        .fetch_all(pool)
        .await;
    match values {
        Ok(values) => Ok(values),
        Err(e) => {
            let msg = format!("Помилка отримання усих значень від БД: {:?}", e);
            printers::err(msg);
            Err(())
        }
    }
}

async fn get_logging_only(pool: &Pool<MySql>) -> Result<Vec<ValueRead>, ()> {
    let values = sqlx::query_as::<_, ValueRead>(
        "SELECT id,
       parent_device_id,
       value_name,
       value_tag,
       description,
       decoding_type,
       settings,
       is_logging
        FROM value_units WHERE is_logging = 1"
    )
        .fetch_all(pool)
        .await;
    match values {
        Ok(values) => Ok(values),
        Err(e) => {
            let msg = format!("Помилка отримання усих значень з логуванням від БД: {:?}", e);
            printers::err(msg);
            Err(())
        }
    }
}

pub async fn delete_value_with_transaction(conn: &mut sqlx::MySqlConnection, id: i32) -> Result<(), String> {
    sqlx::query("Delete FROM measures WHERE value_id = ?")
        .bind(id)
        .execute(&mut *conn)
        .await
        .map_err(|e|{
            let msg = format!("Помилка видалення збережених вимірів id: {} \n{}", id, &e);
            printers::err(msg.clone());
            msg
        })?;

    sqlx::query("DELETE FROM value_units WHERE id = ?")
        .bind(id)
        .execute(&mut *conn)
        .await
        .map_err(|e|{
            let msg = format!("Помилка видалення значення id: {} \n{}", id, &e);
            printers::err(msg.clone());
            msg
        })?;
    Ok(())
}

async fn delete_value(pool: &Pool<MySql>, value: &ValueDelete, tx_to_reader: mpsc::Sender<ConfigEvent>) -> Result<(), String> {

    let deleted_value_old = if let Ok(Some(value)) = get_value_by_id(pool, value.id).await {
        value
    } else {
        let msg = "Помилка читання значення при видаленні".to_string();
        printers::err(msg.clone());
        return Err(msg);
    };

    let mut tx = pool.begin().await.map_err(|e| {
        let msg = format!("Помилка створення батчу при видаленні значення \n{}", &e);
        printers::err(msg.clone());
        msg
    })?;

    delete_value_with_transaction(&mut tx, value.id).await?;

    tx.commit().await.map_err(|e| {
        let msg = format!("Помилка коміту транзакції при видаленні значення id: {} \n{}", value.id, &e);
        printers::err(msg.clone());
        msg
    })?;

    if let Ok(node)= get_node_directly_by_device_id(pool, deleted_value_old.parent_device_id).await {
        let change_config = ConfigEvent {
            event_type: ConfigEventType::Update,
            data: Arc::new(node)
        };
        if let Err(e) = tx_to_reader.send(change_config).await {
            printers::warn(format!("Помилка відправки нової конфігурації в читачі: {}", e))
        }
    } else {
        printers::warn("Помилка отримання ноди для оновлення".to_string());
    }

    let msg = format!("Видалено значення : {:?}", value);
    printers::event(msg);
    Ok(())
}

async fn create_value(pool: &Pool<MySql>, value: &ValueCreate, tx_to_reader: mpsc::Sender<ConfigEvent>) -> Result<(), String> {

    check_json(value.decoding_type, &value.settings.clone().to_string())?;

    let dev = sqlx::query("SELECT id FROM devices WHERE id = ? AND deleted = 0")
        .bind(value.parent_device_id)
        .fetch_optional(pool)
        .await
        .map_err(|e|{
            let msg = format!("Помилка читання з бази даних при перевірці пристрою: \n{}", e);
            printers::err(msg.clone());
            msg
        })?;
    if dev.is_none() {
        let msg = format!("Відсутній пристрій (device) з таким id: {}", value.parent_device_id);
        printers::err(msg.clone());
        return Err(msg)
    };

    sqlx::query(
        "INSERT INTO value_units (parent_device_id, value_name, value_tag, description, decoding_type, settings, is_logging) VALUES (?, ?, ?, ?, ?, ?, ?)"
    )
        .bind(value.parent_device_id)
        .bind(&value.value_name)
        .bind(&value.value_tag)
        .bind(&value.description)
        .bind(value.decoding_type)
        .bind(&value.settings)
        .bind(value.is_logging)
        .execute(pool)
        .await
        .map_err(|e|{
            let msg = format!("Помилка запису в базу даних:\n{}", e);
            printers::err(msg.clone());
            msg
        })?;

    if let Ok(node)= get_node_directly_by_device_id(pool, value.parent_device_id).await {
        let change_config = ConfigEvent {
            event_type: ConfigEventType::Update,
            data: Arc::new(node)
        };
        if let Err(e) = tx_to_reader.send(change_config).await {
            printers::warn(format!("Помилка відправки нової конфігурації в читачі: {}", e))
        }
    } else {
        printers::warn("Помилка отримання ноди для оновлення".to_string());
    }

    let msg = format!("Створено значення : {:?}", value);
    printers::event(msg);

    Ok(())
}
#[derive(Debug, FromRow)]
struct ReadVal{
    id: i32,
    decoding_type: i32,
    is_logging: bool,
}

async fn value_update(pool: &Pool<MySql>, value: &ValueUpdate, tx_to_reader: mpsc::Sender<ConfigEvent>) -> Result<(), String> {

    let value_old = if let Ok(Some(value)) = get_value_by_id(pool, value.id).await {
        value
    } else {
        let msg = "Помилка читання значення при видаленні".to_string();
        printers::err(msg.clone());
        return Err(msg);
    };

    let val = sqlx::query_as::<_, ReadVal>("SELECT id, decoding_type, is_logging FROM value_units WHERE id = ?")
        .bind(value.id)
        .fetch_optional(pool)
        .await
        .map_err(|e|{
            let msg = format!("Помилка читання з бази даних: \n{}", e);
            printers::err(msg.clone());
            msg
        })?;
    let val = if let Some(val) = val {
        val
    } else {
        let msg = format!("Запис (Value) не знайдено, id: {}", value.id);
        printers::err(msg.clone());
        return Err(msg)
    };

    if let Some(setting) = &value.settings {
        let decoding_type = if let Some (decoding_type) = value.decoding_type
        {decoding_type}
        else { val.decoding_type };

        check_json(decoding_type, &setting.to_string())?;
    }

    if let Some(parent_id) = value.parent_device_id {  // може бути bit_in_word, так що унікальність регістра не потрібна
        let dev = sqlx::query("SELECT id FROM devices WHERE id = ? AND deleted = 0")
            .bind(parent_id)
            .fetch_optional(pool)
            .await
            .map_err(|e|{
                let msg = format!("Помилка читання з бази даних при валідації пристрою: \n{}", e);
                printers::err(msg.clone());
                msg
            })?;
        if dev.is_none() {
            let msg = format!("Вказаний пристрій не існує або видалений: \n{}", parent_id);
            printers::err(msg.clone());
            return Err(msg)
        }
    }
    sqlx::query(
        r#"
        UPDATE value_units SET
            parent_device_id = COALESCE(?, parent_device_id),
            value_name = COALESCE(?, value_name),
            value_tag = COALESCE(?, value_tag),
            description = COALESCE(?, description),
            decoding_type = COALESCE(?, decoding_type),
            settings = COALESCE(?, settings),
            is_logging = COALESCE(?, is_logging)
        WHERE id = ?
        "#
    )
        .bind(value.parent_device_id)
        .bind(&value.value_name)
        .bind(&value.value_tag)
        .bind(&value.description)
        .bind(value.decoding_type)
        .bind(&value.settings) // Прямий біндінг Option<Value>, sqlx розбереться самостійно
        .bind(value.is_logging)
        .bind(value.id)
        .execute(pool)
        .await
        .map_err(|e|{
            let msg = format!("Помилка оновлення бази даних (value_units): \n{}", e);
            printers::err(msg.clone());
            msg
        })?;

    let mut new_node_id = 0;

    if let Ok(node)= get_node_directly_by_device_id(pool, value_old.parent_device_id).await {
        new_node_id = node.id;
        let change_config = ConfigEvent {
            event_type: ConfigEventType::Update,
            data: Arc::new(node)
        };
        if let Err(e) = tx_to_reader.send(change_config).await {
            printers::warn(format!("Помилка відправки нової конфігурації в читачі: {}", e))
        }
    } else {
        printers::warn("Помилка отримання ноди для оновлення".to_string());
    };

    if let Some(dev_id) = value.parent_device_id {
        if let Ok(node)= get_node_directly_by_device_id(pool, dev_id).await {
            if new_node_id != node.id {
                let change_config = ConfigEvent {
                    event_type: ConfigEventType::Update,
                    data: Arc::new(node)
                };
                if let Err(e) = tx_to_reader.send(change_config).await {
                    printers::warn(format!("Помилка відправки нової конфігурації в читачі: {}", e))
                }
            }
        } else {
            printers::warn("Помилка отримання ноди для оновлення".to_string());
        }
    }


    let msg = format!("Оновлено значення : {:?}", value);
    printers::event(msg);

    Ok(())
}

async fn get_node_directly_by_device_id(
    pool: &Pool<MySql>,
    device_id: i32
) -> Result<NodeRead, String> {
    let node = sqlx::query_as::<_, NodeRead>(
        r#"
        SELECT n.id, n.ip, n.port, n.description
        FROM nodes n
        INNER JOIN devices d ON n.id = d.parent_node_id
        WHERE d.id = ?
        "#
    )
        .bind(device_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            let msg = format!("Помилка SQL-запиту ноди для девайсу {}: {}", device_id, e);
            printers::err(msg.clone());
            msg
        })?
        .ok_or_else(|| {
            let msg = format!("Не знайдено активної ноди для девайсу з id: {}", device_id);
            printers::err(msg.clone());
            msg
        })?;

    Ok(node)
}