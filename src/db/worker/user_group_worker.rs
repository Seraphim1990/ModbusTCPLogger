use std::collections::{HashMap, HashSet};
use std::ops::Deref;
use sqlx::{MySql, Pool};
use crate::messages::commands::{
    command::{Command, CommandType},
    groups::GroupCommand
};
use crate::logger::printers;
use crate::db::schemas::user_groups::{UserGroupCreate, UserGroupRead, UserGroupUpdate, UiUserGroupRead, UiUserSubGroupsRead};
use crate::messages::requests::group_request::GroupRequest;

pub fn group_command(pool: &Pool<MySql>, command: Command){
    let pool = pool.clone();
    if let CommandType::GroupCommand(group) = command.cmd {
        let tx = command.request_channel;
        match group.as_ref() {
            GroupCommand::Create(_) => {
                let group = group.clone();
                tokio::spawn(async move {
                   if let GroupCommand::Create(group) = group.deref() {
                       run_db_with_timeout(create_group(&pool, group), 5, tx, "GroupCommand::Create").await;
                       /*
                       let res = create_group(&pool, group).await;
                       if tx.send(res).is_err() {
                           printers::err("Помилка відправки калбеку GroupCommand::Create".to_string())
                       }

                        */
                   }
                });
            },
            GroupCommand::Update(_) => {
                let group = group.clone();
                tokio::spawn(async move {
                    if let GroupCommand::Update(group) = group.deref() {
                        run_db_with_timeout(update_group(&pool, group), 5, tx, "GroupCommand::Update").await;
                        /*
                        let res = update_group(&pool, group).await;
                        if tx.send(res).is_err() {
                            printers::err("Помилка відправки калбеку GroupCommand::Update".to_string())
                        }

                         */
                    }
                });
            },
            GroupCommand::Delete(_) => {
                let group = group.clone();
                tokio::spawn(async move {
                    if let GroupCommand::Delete(group) = group.deref() {
                        run_db_with_timeout(delete_group(&pool, group.id), 5, tx, "GroupCommand::Delete").await;
                        /*
                        let res = delete_group(&pool, group.id).await;
                        if tx.send(res).is_err() {
                            printers::err("Помилка відправки калбеку GroupCommand::Delete".to_string())
                        }

                         */
                    }
                });
            },
        }
    }
}

pub fn groups_get(pool: &Pool<MySql>, request: GroupRequest){
    let pool = pool.clone();
    match request {
        GroupRequest::GetById(request) => {
            tokio::spawn(async move {
                let tx = request.request_channel;
                run_db_with_timeout(get_group_by_id(&pool, request.group_id), 3, tx, "GroupRequest::GetById").await;
                /*
                let res = get_group_by_id(&pool, request.group_id).await;
                if tx.send(res).is_err(){
                    printers::err("Помилка відправки калбеку GroupRequest::GetById".to_string())
                }

                 */
            });
        },
        GroupRequest::GetAll(request) => {
            tokio::spawn(async move {
               let tx = request.request_channel;
                run_db_with_timeout(get_all_groups(&pool), 3, tx, "GroupRequest::GetAll").await;
                /*
                let res = get_all_groups(&pool).await;
                if tx.send(res).is_err() {
                    printers::err("Помилка відправки калбеку GroupRequest::GetAll".to_string())
                }

                 */
            });
        },
        GroupRequest::GetByUserId(request) => {
            tokio::spawn(async move {
                let tx = request.request_channel;
                run_db_with_timeout(get_group_by_user_id(&pool, request.user_id), 3, tx, "GroupRequest::GetByUserId").await;
                /*
                let res = get_group_by_user_id(&pool, request.user_id).await;
                if tx.send(res).is_err() {
                    printers::err("Помилка відправки калбеку GroupRequest::GetByUserId".to_string())
                }

                 */
            });
        },
        GroupRequest::GetForUi(request) => {
            tokio::spawn(async move {
                let tx = request.request_channel;
                run_db_with_timeout(ui_get_group(&pool, request.id), 3, tx, "GroupRequest::GetForUi").await;
                /*
                let res = ui_get_group(&pool, request.id).await;
                if tx.send(res).is_err() {
                    printers::err("Помилка відправки калбеку GroupRequest::GetForUi".to_string())
                }

                 */
            });
        }
    }
}

async fn get_group_by_id(pool: &Pool<MySql>, id: i32) -> Result<Option<UserGroupRead>, ()> {
    let group = sqlx::query_as::<_, UserGroupRead>("SELECT id, group_name, description FROM user_groups WHERE id = ?")
        .bind(id)
        .fetch_optional(pool)
        .await
        .map_err(|e|{
            printers::err(format!("Помилка читання бази даних: {}", e));
        })?;

    Ok(group)
}
async fn get_group_by_user_id(pool: &Pool<MySql>, user_id: i32) -> Result<Vec<UserGroupRead>, ()> {
    let groups = sqlx::query_as::<_, UserGroupRead>("SELECT g.id, g.group_name, g.description
                                                    FROM user_groups g
                                                    INNER JOIN user_group_access uga ON g.id = uga.group_id
                                                    WHERE uga.user_id = ?")
        .bind(user_id)
        .fetch_all(pool)
        .await
        .map_err(|e|{
            printers::err(format!("Помилка читання бази даних: {}", e));
        })?;
    Ok(groups)
}

async fn get_all_groups(pool: &Pool<MySql>) -> Result<Vec<UserGroupRead>, ()> {
    let groups = sqlx::query_as::<_, UserGroupRead>("SELECT id, group_name, description FROM user_groups")
        .fetch_all(pool)
        .await
        .map_err(|e|{
            printers::err(format!("Помилка читання бази даних: {}", e));
        })?;
    Ok(groups)
}

async fn create_group(pool: &Pool<MySql>, group: &UserGroupCreate) -> Result<(), String> {
    sqlx::query("INSERT INTO user_groups (group_name, description) VALUES (?, ?)")
        .bind(&group.group_name)
        .bind(&group.description)
        .execute(pool)
        .await
        .map_err(|e| {
            let msg = format!("Помилка створення групи: {}", e);
            printers::err(msg.clone());
            msg
        })?;
    Ok(())
}

async fn update_group(pool: &Pool<MySql>, group: &UserGroupUpdate) -> Result<(), String> {
    sqlx::query("UPDATE user_groups SET group_name = COALESCE(?, group_name), description = COALESCE(?, description) WHERE id = ?")
        .bind(&group.group_name)
        .bind(&group.description)
        .bind(group.id)
        .execute(pool)
        .await
        .map_err(|e|{
            let msg = format!("Помилка оновлення групи: {}", e);
            printers::err(msg.clone());
            msg
        })?;
    Ok(())
}

async fn delete_group(pool: &Pool<MySql>, group_id: i32) -> Result<(), String> {
    sqlx::query("DELETE FROM user_groups WHERE id = ?")
        .bind(group_id)
        .execute(pool)
        .await
        .map_err(|e|{
            let msg = format!("Помилка Видалення групи: {}", e);
            printers::err(msg.clone());
            msg
        })?;
    Ok(())
}

// UI block

use crate::db::schemas::value_unit::ValueRead;
use crate::db::worker::gen_fn::run_db_with_timeout;
/*
async fn ui_get_group(pool: &Pool<MySql>, id: i32) -> Result<Option<UiUserGroupRead>, ()> {

    let subgroups = crate::db::worker::user_sub_group_workers::get_by_group_id(pool, id).await?; // костиль, потом поправлю!!

    if subgroups.is_empty() { return Ok(None) }

    let mut res = UiUserGroupRead { id, sub_groups: Vec::new(), nodes: Vec::new(), devices: Vec::new() };

    for subgroup in subgroups.into_iter() {
        let grouped_values = sqlx::query_as::<_, ValueRead>
            ("SELECT vu.*
            FROM `value_units` vu
            INNER JOIN `subgroup_values` sgv ON vu.`id` = sgv.`value_unit_id`
            WHERE sgv.`subgroup_id` = ?;")
            .bind(subgroup.id)
            .fetch_all(pool)
            .await
            .map_err(|e|{
                printers::err(format!("Помилка отримання значень для підгрупЖ {}", e));
            })?;

        res.sub_groups.push(
            UiUserSubGroupsRead {
                group_state: subgroup,
                values: grouped_values,
            }
        )
    }
    Ok(Some(res))
}

 */
#[derive(sqlx::FromRow)]
struct ValueWithSubgroup {
    subgroup_id: i32,

    id: i32,
    parent_device_id: i32,
    value_name: Option<String>,
    value_tag: String,
    description: Option<String>,
    decoding_type: i32,
    settings: serde_json::Value,
    is_logging: bool,

    node_id: Option<i32>,
}
pub async fn ui_get_group(
    pool: &Pool<MySql>,
    id: i32,
) -> Result<Option<UiUserGroupRead>, ()> {

    let subgroups =
        crate::db::worker::user_sub_group_workers::get_by_group_id(pool, id)
            .await?;

    if subgroups.is_empty() {
        return Ok(None);
    }

    let subgroup_ids: Vec<i32> =
        subgroups.iter().map(|x| x.id).collect();

    let placeholders =
        std::iter::repeat_n("?", subgroup_ids.len())
            .collect::<Vec<_>>()
            .join(",");

    let sql = format!(
        "
        SELECT
            sgv.subgroup_id,

            vu.id,
            vu.parent_device_id,
            vu.value_name,
            vu.value_tag,
            vu.description,
            vu.decoding_type,
            vu.settings,
            vu.is_logging,

            d.parent_node_id as node_id

        FROM subgroup_values sgv

        INNER JOIN value_units vu
            ON vu.id = sgv.value_unit_id

        INNER JOIN devices d
            ON d.id = vu.parent_device_id

        WHERE sgv.subgroup_id IN ({})
        ",
        placeholders
    );

    let mut query =
        sqlx::query_as::<_, ValueWithSubgroup>(&sql);

    for subgroup_id in &subgroup_ids {
        query = query.bind(subgroup_id);
    }

    let rows = query
        .fetch_all(pool)
        .await
        .map_err(|e| {
            printers::err(format!(
                "Помилка отримання значень групи: {}",
                e
            ));
        })?;

    let mut values_by_subgroup:
        HashMap<i32, Vec<ValueRead>> = HashMap::new();

    let mut devices = HashSet::<i32>::new();
    let mut nodes = HashSet::<i32>::new();

    for row in rows {

        devices.insert(row.parent_device_id);

        if let Some(node_id) = row.node_id {
            nodes.insert(node_id);
        }

        let value = ValueRead {
            id: row.id,
            parent_device_id: row.parent_device_id,
            value_name: row.value_name.unwrap_or("".to_string()),
            value_tag: row.value_tag,
            description: row.description,
            decoding_type: row.decoding_type,
            settings: row.settings,
            is_logging: row.is_logging,
        };

        values_by_subgroup
            .entry(row.subgroup_id)
            .or_default()
            .push(value);
    }

    let mut res = UiUserGroupRead {
        id,
        sub_groups: Vec::new(),
        nodes: nodes.into_iter().collect(),
        devices: devices.into_iter().collect(),
    };

    for subgroup in subgroups {

        let values =
            values_by_subgroup
                .remove(&subgroup.id)
                .unwrap_or_default();

        res.sub_groups.push(
            UiUserSubGroupsRead {
                group_state: subgroup,
                values,
            }
        );
    }

    Ok(Some(res))
}