use std::ops::Deref;
use sqlx::{MySql, Pool};
use crate::messages::commands::{
    command::{Command, CommandType},
    asign::{
        AssignGroupsAndValuesCommand,
        AssignGroupsCommand,
        AssignValuesCommand
    },
};
use crate::db::worker::gen_fn::run_db_with_timeout;

pub fn command_assign(pool: &Pool<MySql>, command: Command) {
    let pool = pool.clone();
    if let CommandType::AssignGroupsAndValuesCommand(assign) = command.cmd {
        match assign.as_ref() {
            AssignGroupsAndValuesCommand::Groups(_) => {
                let assign = assign.clone();
                tokio::spawn(async move {
                    if let AssignGroupsAndValuesCommand::Groups(assign) = assign.deref() {
                        let tx = command.request_channel;
                        run_db_with_timeout(assign_groups_to_user(&pool, assign), 5, tx, "AssignGroupsAndValuesCommand::Groups").await;
                        /*
                        match timeout(Duration::from_secs(2), assign_groups_to_user(&pool, assign)).await {
                            Ok(res) => {
                                if tx.send(res).is_err() {
                                    printers::err("Помилка повернення калбеку AssignGroupsAndValuesCommand::Groups".to_string());
                                }
                            }
                            Err(_) => {
                                printers::err("таймаут AssignGroupsAndValuesCommand::Groups".to_string());
                            }
                        }

                         */
                    }
                });
            },
            AssignGroupsAndValuesCommand::Values(_) => {
                let assign = assign.clone();
                tokio::spawn(async move {
                    if let AssignGroupsAndValuesCommand::Values(assign) = assign.deref() {
                        let tx = command.request_channel;
                        run_db_with_timeout(assign_values_to_subgroup(&pool, assign), 5, tx, "AssignGroupsAndValuesCommand::Values").await;
                        /*
                        let res = assign_values_to_subgroup(&pool, assign).await;
                        if tx.send(res).is_err() {
                            printers::err("Помилка повернення калбеку AssignGroupsAndValuesCommand::Values".to_string());
                        }

                         */
                    }
                });
            }
        }
    }
}

async fn assign_groups_to_user(pool: &Pool<MySql>, data: &AssignGroupsCommand) -> Result<(), String> {
    // 1. Починаємо транзакцію
    let mut tx = pool.begin().await.map_err(|e| format!("Помилка старту транзакції: {e}"))?;

    // 2. Видаляємо всі старі прив'язки цього користувача
    sqlx::query("DELETE FROM user_group_access WHERE user_id = ?")
        .bind(data.user_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Помилка очищення старих прав користувача: {e}"))?;

    // 3. Якщо список груп не пустий, записуємо нові зв'язки
    if !data.group_ids.is_empty() {
        // Формуємо динамічний запит для масового інсерту (Bulk Insert)
        // INSERT INTO user_group_access (user_id, group_id) VALUES (?, ?), (?, ?)...
        let mut query_builder = sqlx::QueryBuilder::new("INSERT INTO user_group_access (user_id, group_id) ");

        query_builder.push_values(&data.group_ids, |mut b, &group_id| {
            b.push_bind(data.user_id)
                .push_bind(group_id);
        });

        let query = query_builder.build();
        query.execute(&mut *tx)
            .await
            .map_err(|e| format!("Помилка запису нових прав доступу (можливо, вказано неіснуючий group_id): {e}"))?;
    }

    // 4. Фіксуємо транзакцію
    tx.commit().await.map_err(|e| format!("Помилка коміту транзакції: {e}"))?;
    Ok(())
}

async fn assign_values_to_subgroup(pool: &Pool<MySql>, data: &AssignValuesCommand) -> Result<(), String> {
    // 1. Починаємо транзакцію
    let mut tx = pool.begin().await.map_err(|e| format!("Помилка старту транзакції: {e}"))?;

    // 2. Видаляємо старі регістри з цієї установки
    sqlx::query("DELETE FROM subgroup_values WHERE subgroup_id = ?")
        .bind(data.subgroup_id)
        .execute(&mut *tx)
        .await
        .map_err(|e| format!("Помилка очищення регістрів підгрупи: {e}"))?;

    // 3. Масово додаємо нові регістри, якщо вони є в списку
    if !data.value_unit_ids.is_empty() {
        let mut query_builder = sqlx::QueryBuilder::new("INSERT INTO subgroup_values (subgroup_id, value_unit_id) ");

        query_builder.push_values(&data.value_unit_ids, |mut b, &value_id| {
            b.push_bind(data.subgroup_id)
                .push_bind(value_id);
        });

        let query = query_builder.build();
        query.execute(&mut *tx)
            .await
            .map_err(|e| format!("Помилка прив'язки регістрів (перевірте, чи всі value_unit_id існують): {e}"))?;
    }

    // 4. Фіксуємо зміни
    tx.commit().await.map_err(|e| format!("Помилка коміту транзакції: {e}"))?;
    Ok(())
}