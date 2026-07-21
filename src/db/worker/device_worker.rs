use std::sync::Arc;
use sqlx::{MySql, Pool};
use tokio::sync::mpsc;
use crate::db::schemas::device::*;
use crate::db::schemas::node::NodeRead;
use crate::messages::requests::device_request::*;
use crate::logger::printers;
use crate::messages::commands::command::{Command, CommandType};
use crate::messages::commands::device::DeviceCommand;
use crate::db::worker::value_worker;
use crate::messages::config_event::{ConfigEvent, ConfigEventType};
use crate::db::worker::node_worker::get_node_by_id;
use crate::db::worker::gen_fn::run_db_with_timeout;

pub fn command_device(pool: &Pool<MySql>, command: Command, tx_to_reader: mpsc::Sender<ConfigEvent>) {
    let pool = pool.clone();
    if let CommandType::DeviceCommand(device) = command.cmd  {
        let tx = command.request_channel;
        match device.as_ref() {
            DeviceCommand::Create(_) => {
                let device = device.clone();
                tokio::spawn(async move {
                    if let DeviceCommand::Create(device) = device.as_ref() {
                        run_db_with_timeout(create_device(&pool, device, tx_to_reader), 5, tx, "DeviceCommand::Create").await;
                        /*
                        let res = create_device(&pool, device, tx_to_reader).await;
                        if tx.send(res).is_err() {
                            let msg = "Помилка повернення калбеку DeviceCommand::Create".to_string();
                            printers::err(msg)
                        }

                         */
                    }
                });
            },
            DeviceCommand::Delete(_) => {
                let device = device.clone();
                tokio::spawn(async move {
                    if let DeviceCommand::Delete(device) = device.as_ref() {
                        run_db_with_timeout(delete_device(&pool, device, tx_to_reader), 5, tx, "DeviceCommand::Delete").await;
                        /*
                        let res = delete_device(&pool, device, tx_to_reader).await;
                        if tx.send(res).is_err() {
                            let msg = format!("Помилка повернення калбеку DeviceCommand::Delete id: {}", device.id);
                            printers::err(msg)
                        }
                        
                         */
                    }
                });
            },
            DeviceCommand::Update(_) => {
                let device = device.clone();
                tokio::spawn(async move {
                    if let DeviceCommand::Update(device) = device.as_ref() {
                        run_db_with_timeout(update_device(&pool, device, tx_to_reader), 5, tx, "DeviceCommand::Update").await;
                        /*
                        let res = update_device(&pool, device, tx_to_reader).await;
                        if tx.send(res).is_err() {
                            let msg = format!("Помилка повернення калбеку DeviceCommand::Update id: {}", device.id);
                            printers::err(msg)
                        }
                        
                         */
                    }
                });
            },
        }
    }
}

pub fn devise_get(pool: &Pool<MySql>, request: DeviceRequest) {
    let pool = pool.clone();

    match request {
        DeviceRequest::GetDeviceById(request) => {
            tokio::spawn(async move {
                let tx = request.request_channel;
                run_db_with_timeout(get_device_by_id(&pool, request.id), 3, tx, "DeviceRequest::GetDeviceById").await;
                /*
                let res = get_device_by_id(&pool, request.id).await;
                if tx.send(res).is_err() {
                    let msg = format!("Помилка відправки DeviceRequest::GetDeviceById {}", request.id);
                    printers::warn(msg);
                };
                
                 */
            });
        }
        DeviceRequest::GetByNode(request) => {
            tokio::spawn(async move {
                let tx = request.request_channel;
                run_db_with_timeout(get_device_by_node_id(&pool, request.node_id), 3, tx, "DeviceRequest::GetByNode").await;
                /*
                let res = get_device_by_node_id(&pool, request.node_id).await;
                if  tx.send(res).is_err() {
                    let msg = format!("Помилка відправки DeviceRequest::GetByNode {}", request.node_id);
                    printers::warn(msg);
                };
                
                 */
            });
        }
        DeviceRequest::GetAllDevices(request) => {
            tokio::spawn(async move {
                let tx = request.request_channel;
                run_db_with_timeout(get_all_devices(&pool), 3, tx, "DeviceRequest::GetAllDevices").await;
                /*
                let res = get_all_devices(&pool).await;
                if tx.send(res).is_err() {
                    printers::warn(String::from("Помилка відправки DeviceRequest::GetAllDevices"));
                };
                
                 */
            });
        }
        DeviceRequest::GetDeleted(request) => {
            tokio::spawn(async move {
                let tx = request.request_channel;
                run_db_with_timeout(get_deleted_devices(&pool), 3, tx, "DeviceRequest::GetDeleted").await;
                /*
                let res = get_deleted_devices(&pool).await;
                let send_res = tx.send(res);
                match send_res {
                    Ok(_) => {},
                    Err(_) => {
                        printers::warn(String::from("Помилка відправки DeviceRequest::GetDeleted"));
                    }
                };
                
                 */
            });
        }
    }
}

async fn get_device_by_id(pool: &Pool<MySql>, id: i32) -> Result<Option<DeviceRead>, ()> {
    let device = sqlx::query_as::<_, DeviceRead>(
        "SELECT id,
       parent_node_id,
       device_name,
       address,
       time_for_recall,
       timeout,
       retry_count,
       is_active,
       read_by_group,
       description,
       deleted,
       deleted_at
        FROM devices WHERE id = ? AND deleted = 0",
    ).bind(id).fetch_optional(pool).await;
    match device {
        Ok(Some(device)) => {Ok(Some(device))},
        Ok(None) => {Ok(None)},
        Err(err) => {
            let msg = format!("Помилка отримання пристрою по id: \n{:?}", err);
            printers::err(msg);
            Err(())
        }
    }
}

async fn get_device_by_node_id(pool: &Pool<MySql>, parent_id: i32) -> Result<Vec<DeviceRead>, ()> {
    let devices = sqlx::query_as::<_, DeviceRead>(
        "SELECT id,
       parent_node_id,
       device_name,
       address,
       time_for_recall,
       timeout,
       retry_count,
       is_active,
       read_by_group,
       description,
       deleted,
       deleted_at
        FROM devices WHERE parent_node_id = ? AND deleted = 0"
    )
        .bind(parent_id)
        .fetch_all(pool)
        .await;

    match devices {
        Ok(devices) => Ok(devices),
        Err(err) => {
            let msg = format!("Помилка отримання пристрою по батьківському id: \n{:?}", err);
            printers::err(msg);
            Err(())
        }
    }
}

async fn get_all_devices(pool: &Pool<MySql>) -> Result<Vec<DeviceRead>, ()> {

    let devices = sqlx::query_as::<_, DeviceRead>(
        "SELECT id,
       parent_node_id,
       device_name,
       address,
       time_for_recall,
       timeout,
       retry_count,
       is_active,
       read_by_group,
       description,
       deleted,
       deleted_at
        FROM devices WHERE deleted = 0"
    )
        .fetch_all(pool)
        .await;
    match devices {
        Ok(devices) => { Ok(devices) },
        Err(err) => {
            let msg = format!("Помилка отримання усих пристроїв: \n{:?}", err);
            printers::err(msg);
            Err(())
        }
    }
}

async fn get_deleted_devices(pool: &Pool<MySql>) -> Result<Vec<DeviceRead>, ()> {
    let devices = sqlx::query_as::<_, DeviceRead>(
        "SELECT id,
       parent_node_id,
       device_name,
       address,
       time_for_recall,
       timeout,
       retry_count,
       is_active,
       read_by_group,
       description,
       deleted,
       deleted_at
        FROM devices WHERE deleted = 1"
    ).fetch_all(pool).await;
    match devices {
        Ok(devices) => Ok(devices),
        Err(err) => {
            let msg = format!("Помилка отримання видалених пристроїв: \n{:?}", err);
            printers::err(msg);
            Err(())
        }
    }
}

async fn delete_device(pool: &Pool<MySql>, device: &DeviceDelete, tx_to_reader: mpsc::Sender<ConfigEvent>) -> Result<(), String> {

    let deleted_device = if let Ok(Some(device)) = get_device_by_id(pool, device.id).await {
        device
    } else {
        let msg = format!("Помилка читання пристрою перед видаленням:{}", device.id);
        printers::err(msg.clone());
        return Err(msg.clone())
    };

    let devices_values = value_worker::get_value_by_device_id(pool, device.id)
        .await
        .map_err(|_|{
            let msg = format!("Помилка видалення пристрою id: {}\n Помилка читання значень пристрою", device.id);
            printers::err(msg.clone());
            msg
        })?;

    let mut tx = pool.begin().await.map_err(|e| {
        let msg = format!("Помилка створення батчу при видаленні пристрою \n{}", &e);
        printers::err(msg.clone());
        msg
    })?;

    for value in devices_values {
        value_worker::delete_value_with_transaction(&mut tx, value.id).await
            .map_err(|e|{
            let msg = format!("Помилка видалення пристрою id: {}\n Помилка видалення значень пристрою: \n{}", device.id, &e);
            printers::err(msg.clone());
            msg
        })?;
    }

    sqlx::query("DELETE FROM devices WHERE id = ?")
    .bind(device.id)
    .execute(&mut *tx)
    .await
        .map_err(|err| {
            let msg = format!("Помилка видалення пристрою id: {}\n{}", device.id, err);
            printers::err(msg.clone());
            msg
        })?;
    tx.commit().await.map_err(|err| {
        let msg = format!("Помилка видалення пристрою id: {}\nПомилка коміту транзакції: \n{}", device.id, err);
        printers::err(msg.clone());
        msg
    })?;

    if let Ok(Some(node))= get_node_by_id(pool, deleted_device.parent_node_id).await {
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

    let msg = format!("Видалено пристрій по запиту: {:?}", device);
    printers::event(msg);
    Ok(())
}

async fn create_device(pool: &Pool<MySql>, device: &DeviceCreate, tx_to_reader: mpsc::Sender<ConfigEvent>) -> Result<(), String> {

    if device.address < 0 || device.address > 255 {
        return Err("Не вірна адреса пристрою".to_string());
    }
    if device.time_for_recall <= 0 {
        return Err("Час опитування не може бути від'ємним або нулем".to_string());
    }
    if device.retry_count <= 0 {
        return Err("Кількість повторів не може бути від'ємною або нулем".to_string());
    }
    if device.timeout <= 0 {
        return Err("Час таймауту (timeout) не може бути від'ємним або нулем".to_string());
    }
    if device.parent_node_id <= 0 {
        return Err("id Батьківської ноди не може бути від'ємним".to_string());
    }

    let node_read = sqlx::query_as::<_, NodeRead>("SELECT id, ip, port, description FROM nodes WHERE id = ?")
        .bind(device.parent_node_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| {
            let msg = format!("Помилка отримання ноди від БД по id: {:?}", e);
            printers::err(msg.clone());
            msg
        })?;
    if node_read.is_none() {
        let msg = format!("Помилка створення пристрою, відсутня батьківська нода: {:?}", device);
        printers::err(msg.clone());
        return Err(msg)
    }


    let device_read = sqlx::query_as::<_, DeviceRead>(
        "SELECT id, parent_node_id, device_name, address, time_for_recall, timeout, retry_count, is_active, read_by_group, description, deleted, deleted_at
             FROM devices
             WHERE address = COALESCE(?, address) AND parent_node_id = COALESCE(?, parent_node_id) AND deleted = 0"
    )
        .bind(device.address)
        .bind(device.parent_node_id)
        .fetch_optional(pool)
        .await
        .map_err(|e|{
            let msg = format!("Помилка перевірки унікальності адреси: {}", e);
            printers::err(msg.clone());
            msg
        })?;

    if device_read.is_some() {
        return Err("Ця нода уже має пристрій з такою адресою".to_string());
    }

    let description = match &device.description {
        Some(description) => description,
        None => &"".to_string(),
    };

    sqlx::query(
        "INSERT INTO devices (parent_node_id, device_name, address, time_for_recall, timeout, retry_count, is_active, read_by_group, description) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)"
    )
        .bind(device.parent_node_id)
        .bind(&device.device_name)
        .bind(device.address)
        .bind(device.time_for_recall)
        .bind(device.timeout)
        .bind(device.retry_count)
        .bind(device.is_active)
        .bind(device.read_by_group)
        .bind(description)
        .execute(pool)
        .await
        .map_err(|e| {
            let msg = format!("Помилка створення пристрою: {:?}", e);
            printers::err(msg.clone());
            msg
        })?;

    let msg = format!("Створено пристрій по запиту: {:?}", device);
    printers::event(msg);

    if let Ok(Some(node))= get_node_by_id(pool, device.parent_node_id).await {
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

    Ok(())
}

async fn update_device(pool: &Pool<MySql>, device: &DeviceUpdate, tx_to_reader: mpsc::Sender<ConfigEvent>) -> Result<(), String> {
    if let Some(addr) = device.address && !(0..=255).contains(&addr) {
        return Err("Не вірна адреса пристрою".to_string());
    }
    if let Some(recall) = device.time_for_recall && recall < 0{
        return Err("Час опитування не може бути від'ємним".to_string());
    }
    if let Some(timeout) = device.timeout && timeout < 0 {
        return Err("Час таймауту не може бути від'ємним".to_string());
    }
    if let Some(retry) = device.retry_count && retry < 0 {
        return Err("Кількість повторів не може бути від'ємною".to_string());
    }

    // Якщо прилітає parent_node_id, перевіряємо його на валідність та існування ноди
    if let Some(p_node_id) = device.parent_node_id {
        if p_node_id <= 0 {
            return Err("id Батьківської ноди не може бути від'ємним".to_string());
        }

        let node = sqlx::query_as::<_, NodeRead>(
            "SELECT id, ip, port, description FROM nodes WHERE id = ?"
        )
            .bind(p_node_id)
            .fetch_optional(pool)
            .await
            .map_err(|e|{
                let msg = format!("Помилка бази даних при перевірці ноди: \n{}", e);
                printers::err(msg.clone());
                msg
            })?;
        if node.is_none() {
            return Err("Ноди з таким id не існує".to_string());
        }
    }

    if device.address.is_some() || device.parent_node_id.is_some() {
        let device_read = sqlx::query_as::<_, DeviceRead>(
            "SELECT id, parent_node_id, device_name, address, time_for_recall, timeout, retry_count, is_active, read_by_group, description, deleted, deleted_at
             FROM devices
             WHERE address = COALESCE(?, address) AND parent_node_id = COALESCE(?, parent_node_id) AND deleted = 0"
        )
            .bind(device.address)
            .bind(device.parent_node_id)
            .fetch_optional(pool)
            .await
            .map_err(|e|{
                let msg = format!("Помилка перевірки унікальності адреси: {}", e);
                printers::err(msg.clone());
                msg
            })?;

        if let Some(device_read) = device_read && device_read.id != device.id {
            return Err("Ця нода уже має пристрій з такою адресою".to_string());
        }
    }

    let old_device = if let Ok(Some(old_device)) = get_device_by_id(pool, device.id).await{
        old_device
    } else {
        let msg = format!("Помилка читання пристрою: {}", device.id);
        printers::err(msg.clone());
        return Err(msg)
    };


    sqlx::query(
        "UPDATE devices
         SET
            device_name = COALESCE(?, device_name),
            address = COALESCE(?, address),
            parent_node_id = COALESCE(?, parent_node_id),
            time_for_recall = COALESCE(?, time_for_recall),
            timeout = COALESCE(?, timeout),
            retry_count = COALESCE(?, retry_count),
            is_active = COALESCE(?, is_active),
            read_by_group = COALESCE(?, read_by_group),
            description = COALESCE(?, description)
         WHERE id = ?"
    )
        .bind(device.device_name.clone())
        .bind(device.address)
        .bind(device.parent_node_id)
        .bind(device.time_for_recall)
        .bind(device.timeout)
        .bind(device.retry_count)
        .bind(device.is_active)
        .bind(device.read_by_group)
        .bind(device.description.clone())
        .bind(device.id)
        .execute(pool)
        .await
        .map_err(|e| {
            let msg = format!("Помилка оновлення пристрою:\n {}", e);
            printers::err(msg.clone());
            msg
        })?;

    let mut new_node = 0;
    if let Ok(Some(device_update)) = get_device_by_id(pool, device.id).await{
        if let Ok(Some(node))= get_node_by_id(pool, device_update.parent_node_id).await {
            new_node = node.id;
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

    } else {
        printers::warn("Помилка читаня пристрою при оновленні".to_string());
    }

    if new_node != old_device.parent_node_id {
        if let Ok(Some(node))= get_node_by_id(pool, old_device.parent_node_id).await {
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
    }

    let msg = format!("Оновлено пристрій по запиту: {:?}", device);
    printers::event(msg);

    Ok(())
}