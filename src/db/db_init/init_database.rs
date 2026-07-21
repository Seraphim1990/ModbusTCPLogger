use sqlx::{MySql, MySqlPool, Pool};

pub async fn init_database(pool: &MySqlPool, addr: &String, port: u16, user: &String, pass: &String) -> Result<(), String> {

    let query = "CREATE DATABASE IF NOT EXISTS scada_db_v2";
    send_query(query, pool, "scada_db_v2").await?;
    let conn = format!(
        "mysql://{}:{}@{}:{}/scada_db_v2",
        user,
        pass,
        addr,
        port
    );

    let pool = if let Ok(pool) =  MySqlPool::connect(conn.as_str()).await{
        pool
    } else {
        return Err("Помилка ініціалізацї бази даних".to_string());
    };
    init_roles(&pool).await?;
    init_users(&pool).await?;
    init_user_groups(&pool).await?;
    init_user_group_access(&pool).await?;
    init_user_subgroups(&pool).await?;
    init_nodes(&pool).await?;
    init_devices(&pool).await?;
    init_decoding_type(&pool).await?;
    init_value_units(&pool).await?;
    init_measures(&pool).await?;
    init_subgroup_values(&pool).await?;
    init_refresh_tokens(&pool).await
}
async fn init_value_units(pool: &Pool<MySql>) -> Result<(), String>{
    /*
CREATE TABLE `value_units` (
`id` int NOT NULL AUTO_INCREMENT,
`parent_device_id` int NOT NULL,
`value_name` varchar(100) COLLATE utf8mb4_unicode_ci DEFAULT NULL,
`value_tag` varchar(100) COLLATE utf8mb4_unicode_ci NOT NULL,
`description` text COLLATE utf8mb4_unicode_ci,
`decoding_type` int NOT NULL,
`settings` json NOT NULL,
`is_logging` tinyint(1) NOT NULL,
PRIMARY KEY (`id`),
UNIQUE KEY `value_tag` (`value_tag`),
KEY `parent_device_id` (`parent_device_id`),
KEY `decoding_type` (`decoding_type`),
CONSTRAINT `value_units_ibfk_1` FOREIGN KEY (`parent_device_id`) REFERENCES `devices` (`id`),
CONSTRAINT `value_units_ibfk_2` FOREIGN KEY (`decoding_type`) REFERENCES `decoding_type` (`id`)
) ENGINE=InnoDB AUTO_INCREMENT=121 DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci
 */

    let query = r#"CREATE TABLE IF NOT EXISTS value_units (
  `id` int NOT NULL AUTO_INCREMENT,
  `parent_device_id` int NOT NULL,
  `value_name` varchar(100) COLLATE utf8mb4_unicode_ci DEFAULT NULL,
  `value_tag` varchar(100) COLLATE utf8mb4_unicode_ci NOT NULL,
  `description` text COLLATE utf8mb4_unicode_ci,
  `decoding_type` int NOT NULL,
  `settings` json NOT NULL,
  `is_logging` tinyint(1) NOT NULL,
  PRIMARY KEY (`id`),
  UNIQUE KEY `value_tag` (`value_tag`),
  KEY `parent_device_id` (`parent_device_id`),
  KEY `decoding_type` (`decoding_type`),
  CONSTRAINT `value_units_ibfk_1` FOREIGN KEY (`parent_device_id`) REFERENCES `devices` (`id`),
  CONSTRAINT `value_units_ibfk_2` FOREIGN KEY (`decoding_type`) REFERENCES `decoding_type` (`id`)
) ENGINE=InnoDB AUTO_INCREMENT=121 DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
    "#;
    send_query(query, pool, "value_units").await
}
async fn init_user_subgroups(pool: &Pool<MySql>) -> Result<(), String>{
    /*
CREATE TABLE `user_subgroups` (
`id` int NOT NULL AUTO_INCREMENT,
`group_id` int NOT NULL,
`subgroup_name` varchar(100) COLLATE utf8mb4_unicode_ci NOT NULL,
`description` text CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
PRIMARY KEY (`id`),
KEY `fk_subgroups_group` (`group_id`),
CONSTRAINT `fk_subgroups_group` FOREIGN KEY (`group_id`) REFERENCES `user_groups` (`id`) ON DELETE CASCADE
) ENGINE=InnoDB AUTO_INCREMENT=2 DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci
 */
    let query = r#"CREATE TABLE IF NOT EXISTS user_subgroups (
    `id` int NOT NULL AUTO_INCREMENT,
  `group_id` int NOT NULL,
  `subgroup_name` varchar(100) COLLATE utf8mb4_unicode_ci NOT NULL,
  `description` text CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
  PRIMARY KEY (`id`),
  KEY `fk_subgroups_group` (`group_id`),
  CONSTRAINT `fk_subgroups_group` FOREIGN KEY (`group_id`) REFERENCES `user_groups` (`id`) ON DELETE CASCADE
    )ENGINE=InnoDB AUTO_INCREMENT=2 DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
    "#;
    send_query(query, pool, "user_subgroups").await
}
async fn init_user_groups(pool: &Pool<MySql>) -> Result<(), String>{
    /*
CREATE TABLE `user_groups` (
`id` int NOT NULL AUTO_INCREMENT,
`group_name` varchar(100) COLLATE utf8mb4_unicode_ci NOT NULL,
`description` text CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
PRIMARY KEY (`id`)
) ENGINE=InnoDB AUTO_INCREMENT=2 DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci
 */
    let query = r#"CREATE TABLE IF NOT EXISTS user_groups (
  `id` int NOT NULL AUTO_INCREMENT,
  `group_name` varchar(100) COLLATE utf8mb4_unicode_ci NOT NULL,
  `description` text CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci,
  PRIMARY KEY (`id`)
   ) ENGINE=InnoDB AUTO_INCREMENT=2 DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
    "#;
    send_query(query, pool, "user_groups").await
}
async fn init_user_group_access(pool: &Pool<MySql>) -> Result<(), String>{
    /*
CREATE TABLE `user_group_access` (
`user_id` int NOT NULL,
`group_id` int NOT NULL,
PRIMARY KEY (`user_id`,`group_id`),
KEY `fk_uga_group` (`group_id`),
CONSTRAINT `fk_uga_group` FOREIGN KEY (`group_id`) REFERENCES `user_groups` (`id`) ON DELETE CASCADE,
CONSTRAINT `fk_uga_user` FOREIGN KEY (`user_id`) REFERENCES `users` (`id`) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci
 */
    let query = r#"CREATE TABLE IF NOT EXISTS user_group_access (
  `user_id` int NOT NULL,
  `group_id` int NOT NULL,
  PRIMARY KEY (`user_id`,`group_id`),
  KEY `fk_uga_group` (`group_id`),
  CONSTRAINT `fk_uga_group` FOREIGN KEY (`group_id`) REFERENCES `user_groups` (`id`) ON DELETE CASCADE,
  CONSTRAINT `fk_uga_user` FOREIGN KEY (`user_id`) REFERENCES `users` (`id`) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
    "#;
    send_query(query, pool, "user_group_access").await
}
async fn init_subgroup_values(pool: &Pool<MySql>) -> Result<(), String>{
    /*
    CREATE TABLE `subgroup_values` (
  `subgroup_id` int NOT NULL,
  `value_unit_id` int NOT NULL,
  PRIMARY KEY (`subgroup_id`,`value_unit_id`),
  KEY `fk_sgv_value` (`value_unit_id`),
  CONSTRAINT `fk_sgv_subgroup` FOREIGN KEY (`subgroup_id`) REFERENCES `user_subgroups` (`id`) ON DELETE CASCADE,
  CONSTRAINT `fk_sgv_value` FOREIGN KEY (`value_unit_id`) REFERENCES `value_units` (`id`) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci
     */
    let query = r#"CREATE TABLE IF NOT EXISTS subgroup_values (
  `subgroup_id` int NOT NULL,
  `value_unit_id` int NOT NULL,
  PRIMARY KEY (`subgroup_id`,`value_unit_id`),
  KEY `fk_sgv_value` (`value_unit_id`),
  CONSTRAINT `fk_sgv_subgroup` FOREIGN KEY (`subgroup_id`) REFERENCES `user_subgroups` (`id`) ON DELETE CASCADE,
  CONSTRAINT `fk_sgv_value` FOREIGN KEY (`value_unit_id`) REFERENCES `value_units` (`id`) ON DELETE CASCADE
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
    "#;
    send_query(query, pool, "subgroup_values").await
}
async fn init_nodes(pool: &Pool<MySql>) -> Result<(), String>{
    /*
CREATE TABLE `nodes` (
`id` int NOT NULL AUTO_INCREMENT,
`ip` varchar(100) COLLATE utf8mb4_unicode_ci NOT NULL,
`port` int DEFAULT NULL,
`description` text COLLATE utf8mb4_unicode_ci,
PRIMARY KEY (`id`)
) ENGINE=InnoDB AUTO_INCREMENT=11 DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci
 */
    let query = r#"CREATE TABLE IF NOT EXISTS nodes (
    `id` int NOT NULL AUTO_INCREMENT,
  `ip` varchar(100) COLLATE utf8mb4_unicode_ci NOT NULL,
  `port` int DEFAULT NULL,
  `description` text COLLATE utf8mb4_unicode_ci,
  PRIMARY KEY (`id`)
) ENGINE=InnoDB AUTO_INCREMENT=11 DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
    "#;
    send_query(query, pool, "nodes").await
}

async fn init_measures(pool: &Pool<MySql>) -> Result<(), String>{
    /*
CREATE TABLE `measures` (
`id` int NOT NULL AUTO_INCREMENT,
`value_id` int NOT NULL,
`measure_value` double NOT NULL,
`measure_time` bigint NOT NULL,
PRIMARY KEY (`id`),
KEY `idx_value_time` (`value_id`,`measure_time`),
CONSTRAINT `measures_ibfk_1` FOREIGN KEY (`value_id`) REFERENCES `value_units` (`id`)
) ENGINE=InnoDB AUTO_INCREMENT=1009 DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci
 */
    let query = r#"CREATE TABLE IF NOT EXISTS measures (
  `id` int NOT NULL AUTO_INCREMENT,
  `value_id` int NOT NULL,
  `measure_value` double NOT NULL,
  `measure_time` bigint NOT NULL,
  PRIMARY KEY (`id`),
  KEY `idx_value_time` (`value_id`,`measure_time`),
  CONSTRAINT `measures_ibfk_1` FOREIGN KEY (`value_id`) REFERENCES `value_units` (`id`)
) ENGINE=InnoDB AUTO_INCREMENT=1009 DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
    "#;
    send_query(query, pool, "measures").await
}
async fn init_devices(pool: &Pool<MySql>) -> Result<(), String>{
    /*
    CREATE TABLE `devices` (
  `id` int NOT NULL AUTO_INCREMENT,
  `parent_node_id` int DEFAULT NULL,
  `device_name` varchar(100) COLLATE utf8mb4_unicode_ci DEFAULT NULL,
  `address` int NOT NULL,
  `time_for_recall` int NOT NULL,
  `timeout` int NOT NULL,
  `retry_count` int NOT NULL,
  `is_active` tinyint(1) NOT NULL,
  `read_by_group` tinyint(1) NOT NULL,
  `description` text COLLATE utf8mb4_unicode_ci,
  `deleted` tinyint(1) DEFAULT '0',
  `deleted_at` bigint DEFAULT NULL,
  PRIMARY KEY (`id`),
  KEY `devices_ibfk_1` (`parent_node_id`),
  CONSTRAINT `devices_ibfk_1` FOREIGN KEY (`parent_node_id`) REFERENCES `nodes` (`id`) ON DELETE SET NULL
) ENGINE=InnoDB AUTO_INCREMENT=27 DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci
     */
    let query = r#"CREATE TABLE IF NOT EXISTS devices (
    `id` int NOT NULL AUTO_INCREMENT,
  `parent_node_id` int DEFAULT NULL,
  `device_name` varchar(100) COLLATE utf8mb4_unicode_ci DEFAULT NULL,
  `address` int NOT NULL,
  `time_for_recall` int NOT NULL,
  `timeout` int NOT NULL,
  `retry_count` int NOT NULL,
  `is_active` tinyint(1) NOT NULL,
  `read_by_group` tinyint(1) NOT NULL,
  `description` text COLLATE utf8mb4_unicode_ci,
  `deleted` tinyint(1) DEFAULT '0',
  `deleted_at` bigint DEFAULT NULL,
  PRIMARY KEY (`id`),
  KEY `devices_ibfk_1` (`parent_node_id`),
  CONSTRAINT `devices_ibfk_1` FOREIGN KEY (`parent_node_id`) REFERENCES `nodes` (`id`) ON DELETE SET NULL
) ENGINE=InnoDB AUTO_INCREMENT=27 DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
    "#;
    send_query(query, pool, "devices").await
}

async fn init_decoding_type(pool: &Pool<MySql>) -> Result<(), String>{
    /*
CREATE TABLE `decoding_type` (
`id` int NOT NULL AUTO_INCREMENT,
`decoding_name` varchar(100) COLLATE utf8mb4_unicode_ci DEFAULT NULL,
PRIMARY KEY (`id`)
) ENGINE=InnoDB AUTO_INCREMENT=4 DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci
 */
    let query = r#"CREATE TABLE IF NOT EXISTS decoding_type (
    `id` int NOT NULL AUTO_INCREMENT,
  `decoding_name` varchar(100) COLLATE utf8mb4_unicode_ci DEFAULT NULL,
  PRIMARY KEY (`id`)
) ENGINE=InnoDB AUTO_INCREMENT=4 DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
    "#;
    send_query(query, pool, "decoding_type").await?;

    /*
    INSERT INTO `decoding_type` (`id`, `decoding_name`) VALUES (1, 'ComaShift'), (2, 'SatecDoubleRegistersInt32'), (3, 'BitInWord')
     */
    let query = r#"INSERT INTO `decoding_type` (`id`, `decoding_name`) VALUES (1, 'ComaShift'), (2, 'SatecDoubleRegistersInt32'), (3, 'BitInWord')"#;
    send_query(query, pool, "Додавання типів декодування").await
}

async fn init_users(pool: &Pool<MySql>) -> Result<(), String> {
    /*
    CREATE TABLE `users` (
  `id` int NOT NULL AUTO_INCREMENT,
  `username` varchar(100) COLLATE utf8mb4_unicode_ci NOT NULL,
  `password_hash` varchar(255) COLLATE utf8mb4_unicode_ci NOT NULL,
  `role_id` int NOT NULL,
  `is_active` tinyint(1) NOT NULL DEFAULT '1',
  PRIMARY KEY (`id`),
  UNIQUE KEY `username` (`username`),
  KEY `fk_users_role` (`role_id`),
  CONSTRAINT `fk_users_role` FOREIGN KEY (`role_id`) REFERENCES `roles` (`id`)
) ENGINE=InnoDB AUTO_INCREMENT=2 DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci
     */

    let query = r#"CREATE TABLE IF NOT EXISTS users (
          `id` int NOT NULL AUTO_INCREMENT,
  `username` varchar(100) COLLATE utf8mb4_unicode_ci NOT NULL,
  `password_hash` varchar(255) COLLATE utf8mb4_unicode_ci NOT NULL,
  `role_id` int NOT NULL,
  `is_active` tinyint(1) NOT NULL DEFAULT '1',
  PRIMARY KEY (`id`),
  UNIQUE KEY `username` (`username`),
  KEY `fk_users_role` (`role_id`),
  CONSTRAINT `fk_users_role` FOREIGN KEY (`role_id`) REFERENCES `roles` (`id`)
) ENGINE=InnoDB AUTO_INCREMENT=2 DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;"#;
    send_query(query, pool, "users").await?;

    let query = "INSERT IGNORE INTO `users` (`username`, `password_hash`, `role_id`, `is_active`)
        VALUES ('Harold_Finch', 'L0ng_@dmin_P@ssw0rd!', 1, 1);";

    send_query(query, pool, "Додавання першого адміна").await // створення першого адміна
}
async fn init_roles(pool: &Pool<MySql>) -> Result<(), String> {
    /*
    CREATE TABLE `roles` (
  `id` int NOT NULL AUTO_INCREMENT,
  `role_name` varchar(50) COLLATE utf8mb4_unicode_ci NOT NULL,
  PRIMARY KEY (`id`),
  UNIQUE KEY `role_name` (`role_name`)
) ENGINE=InnoDB AUTO_INCREMENT=5 DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci
     */
    let query = r#"CREATE TABLE IF NOT EXISTS roles (
    `id` int NOT NULL AUTO_INCREMENT,
  `role_name` varchar(50) COLLATE utf8mb4_unicode_ci NOT NULL,
  PRIMARY KEY (`id`),
  UNIQUE KEY `role_name` (`role_name`)
) ENGINE=InnoDB AUTO_INCREMENT=5 DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
    "#;
    send_query(query, pool, "roles").await?;
    /*
INSERT INTO `roles` (`role_name`) VALUES ('admin'), ('superuser'), ('user'), ('servise')
ON DUPLICATE KEY UPDATE `role_name`=`role_name`;
 */
    let query = r#"INSERT INTO `roles` (`id`, `role_name`)
    VALUES ('1', 'admin'), ('2', 'superuser'), ('3','user'), ('4','servise')
    ON DUPLICATE KEY UPDATE `role_name` = `role_name`;"#;
    send_query(query, pool, "Створення ролей").await
}

async fn init_refresh_tokens(pool: &Pool<MySql>) -> Result<(), String> {
    /*
    CREATE TABLE refresh_tokens (
    user_id     BIGINT PRIMARY KEY,
    user_role_id INT NOT NULL, -- Додаємо поле для ID ролі
    token_hash  VARCHAR(255) NOT NULL, -- Для InnoDB краще VARCHAR замість TEXT, якщо це індекс або UUID
    created_at  BIGINT NOT NULL,
    expires_at  BIGINT NOT NULL
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
     */

    let query = r#"
    CREATE TABLE IF NOT EXISTS refresh_tokens (
    user_id     BIGINT PRIMARY KEY,
    user_role_id INT NOT NULL, -- Додаємо поле для ID ролі
    token_hash  VARCHAR(255) NOT NULL, -- Для InnoDB краще VARCHAR замість TEXT, якщо це індекс або UUID
    created_at  BIGINT NOT NULL,
    expires_at  BIGINT NOT NULL
    )ENGINE=InnoDB AUTO_INCREMENT=5 DEFAULT CHARSET=utf8mb4 COLLATE=utf8mb4_unicode_ci;
    "#;
    send_query(query, pool, "Створення токенів").await
}

async fn send_query(query: &str, pool: &Pool<MySql>, msg: &str) -> Result<(), String> {
    sqlx::query(query).execute(pool).await.map_err(|e| {
        format!("Помилка бази даних:{}\n {}",msg, e)
    })?;
    Ok(())
}