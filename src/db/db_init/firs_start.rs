use std::path::Path;
use std::fs;
use std::net::Ipv4Addr;
use crate::db::states::{DBConf};
use crate::db::db_init::code_decode::*;
use crate::db::db_init::init_database;
use tokio::io::{self, AsyncBufReadExt, BufReader};
use sqlx::MySqlPool;

pub async fn init_db_if_need_it() {
    if !Path::new("configs/db.toml").exists() {
        create_config().await;
    }

    'main_loop: loop {
        let config = get_config().await;
        let url = format!(
            "mysql://{}:{}@{}:{}",
            config.db_user, config.db_password, config.db_address, config.db_port
        );
        match MySqlPool::connect(url.as_str()).await {
            Ok(pool) => {
                let url = format!(
                    "mysql://{}:{}@{}:{}/scada_db_v2",
                    config.db_user, config.db_password, config.db_address, config.db_port
                );
                if MySqlPool::connect(url.as_str()).await.is_err() {
                    match init_database::init_database(&pool, &config.db_address, config.db_port, &config.db_user, &config.db_password).await {
                        Ok(_) => {
                            println!("Базу даних ініціалізовано.\nЛогін адміна: Harold_Finch\nПароль: L0ng_@dmin_P@ssw0rd!\nВ паролі використані нулі, не зпутай буквою 'О'\nЗапиши чи запам'ятай\n");
                            let _ = read_line("Натисни Enter").await;
                            print!("{esc}[2J{esc}[1;1H", esc = 27 as char);
                            break 'main_loop;
                        },
                        Err(e) => {
                            print!("{esc}[2J{esc}[1;1H", esc = 27 as char); // очищення консолі
                            println!("---------------------------------------------------------------------------");
                            println!("{}", e);
                            println!("---------------------------------------------------------------------------");
                            panic!()
                        }
                    }
                } else {break 'main_loop;}
            },
            Err(e) => {
                println!(
                    "Помилка під'єднання до БД\nПеревір конфігурацію:\nuser: {}\naddr: {}\nport: {}\n{}",
                    config.db_user, config.db_address, config.db_port, e
                );
                println!("Якщо в помилках є щось типу\nconnection failed: authentication error\nабо\naccess denied for user 'xxx'\nМожливо не вірний пароль");
                'input_loop: loop {
                    let inp = read_line("Якщо файл конфігурації вірний натисни 'Y', якщо ні натисни 'N'").await;
                    match inp.to_lowercase().as_str() {
                        "y" => {
                            let _ = read_line("Якщо конфігурація вірна, перевір чи запущена СУБД потім натисни Enter").await;
                            continue 'main_loop;
                        },
                        "n" => {
                            create_config().await;
                            break 'input_loop;
                        },
                        _ => {
                            println!("Ввід має бути тільки Y' та 'N'");
                        }
                    }
                }
            }
        }
    }
}

async fn get_config() -> DBConf {
    loop {
        let content = fs::read_to_string("configs/db.toml").expect("Помилка читання configs/db.toml");
        match decrypt_string(content.as_str()) {
            Ok(decoded) => {
                match toml::from_str::<DBConf>(&decoded) {
                    Ok(conf) => {
                        return conf;
                    },
                    Err(_) => {
                        println!("Помилка Парсингу файлу налаштувань БД, давай створимо новий");
                        create_config().await;
                    }
                }
            },
            Err(_) => {
                println!("Помилка декодування файлу налаштувань БД, давай створимо новий");
                create_config().await;
            }
        }
    }
}
async fn create_config() {
    let mut db_user: String;
    let mut db_password: String;
    let mut db_address: String;
    let mut db_port: u16;
    println!("Схоже, що файл конфігурації бази даних відсутній або пошкоджено. Давай проведемо ініціалізацію. Натисни Enter");
    'main_loop: loop {
        db_user = read_line("Ім'я користувача: ").await;
        db_password = read_line("Пароль користувача: ").await;

        'address_loop: loop {
            db_address = read_line("Адреса бази даних: ").await;
            if db_address.parse::<Ipv4Addr>().is_ok() {
                break 'address_loop;
            } else {
                println!("Невірний формат IpV4 адреси");
            }
        }
        'port_loop: loop {
            let port = read_line("Порт бази даних: ").await;

            if let Ok(port) = port.parse::<i32>() && (0..=65535).contains(&port) {
                db_port = port as u16;
                break 'port_loop;
            }
            println!("Число має бути в діапазоні 0-65535\n");
        }
        // наче усе ок, пробуємо під'єднатися
        let url = format!("mysql://{}:{}@{}:{}", &db_user, &db_password, &db_address, db_port);
        match MySqlPool::connect(url.as_str()).await {
            Ok(_) => {
                let db_config = DBConf {
                    db_user,
                    db_password,
                    db_address,
                    db_port
                };
                let db_conf_str = toml::to_string(&db_config).expect("Помилка серіалізації конфігу бази даних"); // це на старті, має вижити, інакше не страшно...
                let encoded = encrypt_string(&db_conf_str).expect("Помилка серіалізації конфігу бази даних");
                fs::create_dir_all("configs").expect("Не вдалося створити каталог");
                fs::write("configs/db.toml", encoded).expect("Помилка запису файлу: {}");
                break 'main_loop;
            }
            Err(e) => {
                println!(
                    "Помилка під'єднання до БД\nПеревір конфігурацію:\nuser: {}\naddr: {}\nport: {}\n{}",
                    db_user, db_address, db_port, e
                );
                println!("Якщо в помилках є щось типу\nconnection failed: authentication error\nабо\naccess denied for user 'xxx'\nМожливо не вірний пароль");

                let _ = read_line("Не вдалось підключитись до бази даних\nПеревірте чи піднята СУБД та вірність даних\nНатисни Enter").await;
                print!("{esc}[2J{esc}[1;1H", esc = 27 as char); // очищення консолі
                continue 'main_loop;
            }
        }
    }
}

async fn read_line(prompt: &str) -> String {
    loop {
        println!("{prompt}");

        let stdin = io::stdin();
        let mut reader = BufReader::new(stdin);
        let mut input = String::new();

        if reader.read_line(&mut input).await.is_err() {
            println!("\nПомилка читання!");
            continue;
        }

        println!();
        return input.trim().to_string()
    }
}