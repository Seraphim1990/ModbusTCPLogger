use std::time::{SystemTime, UNIX_EPOCH};
use sqlx::{MySql, Pool};
use crate::db::schemas::node::*;
use crate::messages::requests::node_request::*;
use crate::messages::commands::{
    command::{Command, CommandType},
    node::NodeCommand
};
use crate::logger::printers;
use std::net::Ipv4Addr;
use std::sync::Arc;
use tokio::sync::mpsc;
use crate::db::worker::gen_fn::run_db_with_timeout;
use crate::messages::config_event::{ConfigEvent, ConfigEventType};

pub fn command_node(pool: &Pool<MySql>, command: Command, tx_to_reader: mpsc::Sender<ConfigEvent>) {
    let pool = pool.clone();
    if let CommandType::NodeCommand(node) = command.cmd {
        let tx = command.request_channel;
        match node.as_ref() {
            NodeCommand::Delete(_) => {
                let node = node.clone();
                tokio::spawn(async move {
                    if let NodeCommand::Delete(node) = node.as_ref() {
                        run_db_with_timeout(delete_node(&pool, node, tx_to_reader), 5, tx, "NodeCommand::Delete").await;
                        /*
                        let res = delete_node(&pool, node, tx_to_reader).await;
                        if tx.send(res).is_err() {
                            let msg = format!("Помилка відправки калбеку NodeCommand::Delete id: {}", node_id);
                            printers::err(msg);
                        }

                         */
                    }
                });
            }
            NodeCommand::Create(_) => {
                let node = node.clone();
                tokio::spawn(async move {
                   if let NodeCommand::Create(node) = node.as_ref() {
                       run_db_with_timeout(create_node(&pool, node, tx_to_reader), 5, tx, "NodeCommand::Create").await;
                       /*
                       let res = create_node(&pool, node, tx_to_reader).await;
                       if tx.send(res).is_err() {
                           let msg = "Помилка відправки калбеку NodeCommand::Create".to_string();
                           printers::err(msg);
                       }

                        */
                    }
                });
            },
            NodeCommand::Update(_) => {
                let node = node.clone();
                tokio::spawn(async move {
                    if let NodeCommand::Update(node) = node.as_ref() {
                        run_db_with_timeout(update_node(&pool, node, tx_to_reader), 5, tx, "NodeCommand::Update").await;
                        /*
                        let node_id = node.id;
                        let res = update_node(&pool, node, tx_to_reader).await;
                        if tx.send(res).is_err() {
                            let msg =  format!("Помилка відправки калбеку NodeCommand::Update id: {}", node_id);
                            printers::err(msg);
                        }

                         */
                    }
                });
            },
        }
    }
}

pub fn node_get(pool: &Pool<MySql>, request: NodeRequest) {
    let pool = pool.clone();
    match request {
        NodeRequest::GetById(request) => {
            tokio::spawn(async move {
                let tx = request.request_channel;
                run_db_with_timeout(get_node_by_id(&pool, request.node_id), 3, tx, "NodeRequest::GetById").await;
                /*
                let res = get_node_by_id(&pool, request.node_id).await;
                if tx.send(res).is_err() {
                    let msg = format!("Помилка відправки NodeRequest::GetNode {}", request.node_id);
                    printers::warn(msg);
                };

                 */
            });
        },
        NodeRequest::GetAll(request) => {
            tokio::spawn(async move {
                let tx = request.request_channel;
                run_db_with_timeout(get_all_node(&pool), 3, tx, "NodeRequest::GetAll").await;
                /*
                let res = get_all_node(&pool).await;
                if tx.send(res).is_err() {
                    printers::warn(String::from("Помилка відправки NodeRequest::GetAll"));
                };

                 */
            });
        }
        NodeRequest::GetByIp(request) => {
            tokio::spawn(async move {
                let tx = request.request_channel;
                run_db_with_timeout(get_by_ip(&pool, &request.node_ip), 3, tx, "NodeRequest::GetByIp").await;
                /*
                let res = get_by_ip(&pool, &request.node_ip).await;
                if tx.send(res).is_err() {
                    let msg = format!("Помилка відправки NodeRequest::GetByIp{}", request.node_ip);
                    printers::warn(msg);
                };

                 */
            });
        }
    }
}

async fn get_all_node(pool: &Pool<MySql>)-> Result<Vec<NodeRead>, ()> {
    let nodes = sqlx::query_as::<_, NodeRead>("SELECT id, ip, port, description FROM nodes")
        .fetch_all(pool)
        .await
        .map_err(|e| {
            let msg = format!("Помилка отримання ноди від БД: {:?}", e);
            printers::err(msg);
        })?;
    Ok(nodes)
}
// Get
pub async fn get_node_by_id(pool: &Pool<MySql>, id: i32)-> Result<Option<NodeRead>, ()> {
    let node = sqlx::query_as::<_, NodeRead>("SELECT id, ip, port, description FROM nodes WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            let msg = format!("Помилка отримання ноди від БД по id: {:?}", e);
            printers::err(msg);
        })?;
    Ok(node)
}

async fn get_by_ip(pool: &Pool<MySql>, ip: &String)-> Result<Option<NodeRead>, ()> {
    let node = sqlx::query_as::<_, NodeRead>("SELECT id, ip, port, description FROM nodes WHERE ip = ?")
        .bind(ip)
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            let msg = format!("Помилка отримання ноди від БД по ip: {:?}", e);
            printers::err(msg);
        })?;
    Ok(node)
}

// Del
async fn delete_node(pool: &Pool<MySql>, node: &NodeDelete, tx_to_reader: mpsc::Sender<ConfigEvent>) -> Result<(), String> {

    let err_closure = |e| {
        let msg = format!("Помилка транзакції при видаленні ноди: {:?}", e);
        printers::err(msg.clone());
        msg
    };


    let mut tx = pool.begin().await
        .map_err(err_closure)?;

    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_secs();

    sqlx::query(
        "UPDATE devices d
            SET d.deleted = 1,
            d.deleted_at = ?
            WHERE d.parent_node_id = ?
            AND EXISTS (
                SELECT 1
                FROM value_units vu
                WHERE vu.parent_device_id = d.id
            AND vu.is_logging = 1
            )"
        )
        .bind(ts)
        .bind(node.id)
        .execute(&mut *tx)
        .await
        .map_err(err_closure)?;

    sqlx::query("
        DELETE vu
        FROM value_units vu
       JOIN devices d ON d.id = vu.parent_device_id
        WHERE d.parent_node_id = ?
        AND deleted = 0")
        .bind(node.id)
        .execute(&mut *tx)
        .await
        .map_err(err_closure)?;


    sqlx::query("
        DELETE FROM devices
        WHERE parent_node_id = ?
        AND deleted = 0;")
        .bind(node.id)
        .execute(&mut *tx)
        .await
        .map_err(err_closure)?;

    let deleted_node = get_node_by_id(pool, node.id)
        .await
        .map_err(|_|{
            let msg = format!("Помилка видалення ноди: {}", node.id);
            printers::err(msg.clone());
            msg
        })?;

    sqlx::query("
        DELETE FROM nodes
        WHERE id = ?;")
        .bind(node.id)
        .execute(&mut *tx)
        .await
        .map_err(err_closure)?;

    tx.commit().await
        .map_err(|e| {
            let msg = format!("Помилка коміту при видаленні ноди: {:?}", e);
            printers::err(msg.clone());
            msg
        })?;

    let change_config = ConfigEvent {
        event_type: ConfigEventType::Delete,
        data: Arc::new(deleted_node.unwrap())
    };

    if let Err(e) = tx_to_reader.send(change_config).await {
        printers::warn(format!("Помилка відправки нової конфігурації в читачі: {}", e))
    }

    let msg = format!("Видалено ноду по запиту: {:?}", node);
    printers::event(msg);
    Ok(())
}

// Create

async fn create_node(pool: &Pool<MySql>, node: &NodeCreate, tx_to_reader: mpsc::Sender<ConfigEvent>) -> Result<(), String> {
    let ip = &node.ip;
    let _ = ip.parse::<Ipv4Addr>().map_err(|e|{
        let msg = format!("Помилка валідації ip адреси: {:?}", e);
        printers::err(msg.clone());
        msg
    })?;

    let node_in_db = get_by_ip(pool, ip).await.map_err(|_|{ // я в курсі, в консолі та файлі все буде написано, інфа про помилку не втрачена!
        let msg = String::from("Помилка перевірки ip адреси в базі даних:");
        printers::err(msg.clone());
        msg
    })?;

    if let Some(node_in_db) = node_in_db {
        let msg = format!("Ip: {} вже існує в базі даних, id: {}", node_in_db.ip, node_in_db.id);
        printers::err(msg.clone());
        return Err(msg)
    };

    let description = match &node.description {
        Some(description) => description,
        None => &"".to_string(),
    };

    sqlx::query("INSERT INTO nodes (ip, port, description) VALUES (?, ?, ?)")
        .bind(ip)
        .bind(node.port.unwrap_or(502))
        .bind(description)
        .execute(pool)
        .await
        .map_err(|e|{
            let msg = format!("Помилка валідації ip адреси: {:?}", e);
            printers::err(msg.clone());
            msg
        })?;

    if let Ok(Some(node))= get_by_ip(pool, &node.ip).await {
        let change_config = ConfigEvent {
            event_type: ConfigEventType::Create,
            data: Arc::new(node)
        };
        if let Err(e) = tx_to_reader.send(change_config).await {
            printers::warn(format!("Помилка відправки нової конфігурації в читачі: {}", e))
        }
    } else {
        printers::warn("Помилка отримання ноди для оновлення".to_string());
    }

    let msg = format!("Створено ноду по запиту: {:?}", node);
    printers::event(msg);
    Ok(())
}

async fn update_node(pool: &Pool<MySql>, node: &NodeUpdate, tx_to_reader: mpsc::Sender<ConfigEvent>) -> Result<(), String> {

    let _ = sqlx::query_as::<_, NodeRead>("SELECT id, ip, port, description FROM nodes WHERE id = ?")
        .bind(node.id)
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            let msg = format!("Помилка знаходження ноди : {}\n{}", node.id, e);
            printers::err(msg.clone());
            msg
        })?
        .ok_or_else(||{
            let msg = format!("Не знайдено ноду з таким id : {}", node.id);
            printers::err(msg.clone());
            msg
        })?;

    if let Some(ip) = &node.ip {
        let _ = ip.parse::<Ipv4Addr>().map_err(|e|{
            let msg = format!("Помилка валідації ip адреси: {:?}", e);
            printers::err(msg.clone());
            msg
        })?;

        let node_in_db = get_by_ip(pool, ip).await.map_err(|_|{ // я в курсі, в консолі та файлі все буде написано, інфа про помилку не втрачена!
            let msg = String::from("Помилка перевірки ip адреси в базі даних:");
            printers::err(msg.clone());
            msg
        })?;

        if let Some(node_in_db) = node_in_db
            && node_in_db.id != node.id
        {
            let msg = format!(
                "Нода з ip: {} вже існує. id: {}",
                node_in_db.ip,
                node_in_db.id
            );
            printers::err(msg.clone());
            return Err(msg);
        }
    }

    let description = match &node.description {
        Some(description) => description,
        None => &"".to_string(),
    };

    sqlx::query(
        "UPDATE nodes SET ip = COALESCE(?, ip), port = COALESCE(?, port), description = COALESCE(?, description) WHERE id = ?"
    )
        .bind(&node.ip)
        .bind(node.port)
        .bind(description)
        .bind(node.id)
        .execute(pool)
        .await
        .map_err(|e|{
            let msg = format!("Помилка оновлення ноди, id: {}\n{}", node.id, e);
            printers::err(msg.clone());
            msg
        })?;

    if let Ok(Some(node))= get_node_by_id(pool, node.id).await {
        let change_config = ConfigEvent {
            event_type: ConfigEventType::Update,
            data: Arc::new(node)
        };
        if let Err(e) = tx_to_reader.send(change_config).await {
            printers::warn(format!("Помилка відправки нової конфігурації в читачі: {}", e))
        }
    }

    let msg = format!("Оновлено ноду по запиту: {:?}", node);
    printers::event(msg);
    Ok(())
}