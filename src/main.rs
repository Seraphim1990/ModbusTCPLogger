#![allow(clippy::clone_on_copy)]
#![allow(clippy::wrong_self_convention)]
#![allow(clippy::result_large_err)]

use std::time::Duration;
use messages::main_msg::MainMsg;
use tokio::io::{AsyncBufReadExt, BufReader as TokioBufReader};
use tokio::sync::mpsc;
use crate::logger::printers;
use crate::messages::config_event::ConfigEvent;

use tokio::time::{sleep_until, Instant};
use chrono::{Local, Days};

pub mod db;
pub mod logger;
mod reader;
mod api;
pub mod messages;
mod data_master;
mod minimal_copy_safe;


#[tokio::main]
async fn main() {
    #[cfg(windows)]
    let _ = colored::control::set_virtual_terminal(true);
    use std::panic;

    panic::set_hook(Box::new(|info| {
        let message = match info.payload().downcast_ref::<&str>() {
            Some(s) => *s,
            None => match info.payload().downcast_ref::<String>() {
                Some(s) => s.as_str(),
                None => "Невідома помилка",
            }
        };

        let saitama = r#"⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⣀⣰⠤⠤⠤⠤⣤⣀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢀⡤⠞⠛⠉⠉⠛⠓⠦⣄⠀⠀⠀
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⣠⣴⣛⣛⣿⡿⠂⠀⠀⠀⠈⠙⠶⣄⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⣠⠞⠁⠀⠀⠀⠀⠀⠀⠀⠈⢳⡀⠀
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⣠⣞⣓⣒⣒⣒⣒⠀⠀⠀⠀⠀⠀⠀⠀⠈⢳⡀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⣰⠏⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢳⠀
⠀⠀⠀⠀⠀⠀⠀⠀⠀⣴⠿⠶⠶⠶⢶⣒⣀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠹⡄⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢠⠏⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠈⡇
⠀⠀⠀⠀⠀⠀⠀⠀⣸⠁⠀⠀⠀⣐⣒⣒⣛⠷⠆⠀⠀⠀⠀⠀⠀⠀⠀⠀⢹⡀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⡾⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢳
⠀⠀⠀⠀⠀⠀⠀⢠⡯⣤⣥⣰⣶⣖⣞⣓⣛⣛⡁⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⣧⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⡇⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢸
⠀⠀⠀⠀⠀⠀⠀⢸⠿⠿⠿⠿⠿⠿⠿⠟⠳⠤⠿⠒⠶⠶⠀⠀⠠⢤⡤⠤⠄⢹⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢸⠃⠀⠀⠀⢀⣤⣤⡄⡄⢀⣤⠀⠀⠀⠀⢸
⠀⠀⠀⠀⠀⠀⠀⢸⡯⣭⠭⠭⠭⠉⠁⢠⡒⠛⠉⡍⢳⡆⠀⣠⡖⠻⡏⠙⣦⢼⠁⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢸⠀⠀⠀⠀⡟⠁⢀⣿⣧⣞⡁⠀⠀⠀⠀⢸
⠀⠀⠀⠀⠀⠀⢠⣟⣻⣻⣿⣫⣭⣭⠽⠌⠛⠢⠤⠔⠋⠀⠀⡇⠓⠦⠥⠞⠁⢸⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⣼⠀⠀⠀⠀⠳⠶⠛⠁⠇⠀⠙⠂⠀⠀⠀⢸
⠀⠀⠀⠀⠀⠀⣸⠛⠛⣷⠛⠲⠦⠤⠭⠿⠦⠄⠀⠀⠀⠀⠀⣧⠀⠀⠀⠀⠀⢸⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠈⠛⣆⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⣸
⠀⠀⠀⠀⠀⠀⢿⡭⢽⢿⡄⠀⠀⢉⣉⡉⠉⠉⠀⠀⠀⠀⢠⣼⡆⠀⠀⠀⠀⢸⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢸⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⡇
⠀⠀⠀⠀⠀⠀⠈⢿⣙⣾⣓⣀⣐⣒⣒⣒⣀⠀⠀⠀⠀⠀⠀⠈⠀⠀⠀⠀⠀⢸⡄⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢸⡆⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢸⠃
⠀⠀⠀⠀⠀⠀⠀⠀⠙⠶⡶⡏⠉⣉⣑⣒⣒⣒⡂⠀⠀⠐⢶⠶⠆⠀⠀⠀⠀⡾⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠘⣷⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢀⡏⠀
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢻⡄⠠⠴⠖⠖⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⣼⠃⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠸⣆⠀⠀⠀⠀⠀⠀⠀⠀⠀⢀⡞⠀⠀
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠻⣏⠉⠉⠉⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⡼⠃⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠈⠳⢤⣀⠀⠀⠀⢀⣠⠔⠋⠀⠀⠀
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠙⡿⣭⡉⠉⠵⠆⠀⠀⠀⠀⢀⣠⠞⠁⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠈⠉⠉⠉⠉⠀⠀⠀⠀⠀⠀
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⣿⠼⠿⠿⣿⣯⡥⠤⠴⠖⣿⣅⡀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀
⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⢀⣴⣾⣿⢿⠛⠛⠛⠉⠉⠀⠀⠀⢿⡈⠙⣍⠹⡦⣄⣀⣤⡀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀
⠀⠀⠀⠀⠀⠀⠀⢀⣤⣴⣾⣭⣿⣯⣿⣭⠥⠄⠀⠀⠀⠀⠀⠀⠾⢷⡀⠘⣦⣽⣬⣧⠀⡻⣦⣄⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀
⠀⠀⠀⠀⢀⣴⣶⣿⣾⣛⣻⣿⣿⣿⣿⡟⠻⣥⣀⡀⠀⠀⢀⣀⡴⠋⠹⣄⠸⣿⣿⣿⣷⠃⢈⣽⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀
⠀⠀⠀⠀⣿⠾⣿⣭⠿⠿⣿⣿⣿⣿⣣⣿⠄⠀⠈⠉⣽⣽⣻⢷⡀⠀⢀⡾⠳⢬⣽⣯⡧⠴⠋⢹⡄⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀
⠀⠀⠀⣼⡧⠄⠉⡉⡿⠿⣿⡿⠿⠿⠯⠭⠿⣦⣄⣀⣻⡌⠁⠸⣥⡞⠋⠀⠀⠀⠀⠀⠀⢀⠀⠈⣧⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀
⠀⠀⡼⣹⠀⡂⠐⠶⠀⠤⢼⣏⣵⣈⣉⠉⠉⠀⠀⢹⣯⣀⢛⣠⡟⡇⠀⠀⠀⠀⠀⠀⠀⣼⣃⠀⢹⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀
⠀⣼⣭⣯⣈⣉⣉⣉⠀⠀⠈⣿⣓⡂⠀⠀⠀⠀⠀⠀⡇⠉⡯⣗⠀⡇⠀⠀⠀⠀⠀⠀⢠⣷⠋⠀⢸⡀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀
⢠⣯⢭⡯⠭⠭⠭⣥⠤⠀⢰⣿⣟⠒⠒⠒⠀⠀⠀⠀⡇⠀⡯⡗⠀⡇⠀⠀⠀⠀⠀⠀⢸⡁⠀⠀⠈⣷⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀⠀"#;

        // Білий фон + чорний текст
        let white_bg = "\x1b[47m\x1b[30m";   // білий фон, чорний текст
        let reset = "\x1b[0m";

        println!(
            "{}{}{}\n🔥 КРИТИЧНА ПОМИЛКА\n{}\n\nНатисни Enter для перезапуску...",
            white_bg,
            saitama.trim_end(),
            reset,
            &message,
        );
        printers::debug(message.to_string());
    }));

    minimal_copy_safe::copy_check::check_key_and_activate();

    api::sockets::check_and_create_server::init_server_config_first_run().await;

    db::db_init::firs_start::init_db_if_need_it().await;

    print!("{esc}[2J{esc}[1;1H", esc = 27 as char);

    loop {

        let (to_api_tx, to_api_rx) = mpsc::channel::<MainMsg>(100);
        let (from_api_tx, from_api_rx) = mpsc::channel::<MainMsg>(100);
        let (from_reader_tx, from_reader_rx) = mpsc::channel::<MainMsg>(100);
        let (to_reader_tx, to_reader_rx) = mpsc::channel::<ConfigEvent>(100);
        let (to_db_tx, to_db_rx) = mpsc::channel::<MainMsg>(100);
        let (from_db_tx, from_db_rx) = mpsc::channel::<ConfigEvent>(100);

        printers::init_logger();

        let mut api_handler = tokio::spawn(async move {
            api::init_axum::init_axum(from_api_tx, to_api_rx).await;
        });

        let mut db_handler= tokio::spawn(async move {
            db::worker::db_master::run_db_master(to_db_rx, from_db_tx).await  // база даних
        });

        let mut data_master_handler = tokio::spawn(async move {
            data_master::data_master_loop::run_data_master(from_api_rx, to_api_tx, from_reader_rx, to_reader_tx, to_db_tx, from_db_rx).await; // контролер
        });

        let mut reader_handler = tokio::spawn(async move {
            reader::reader_loop::reading_loop::read_loop::node_master(to_reader_rx, from_reader_tx).await; // модбас опитувач
        });

        tokio::select! {

            api_drop = &mut api_handler => {
                match api_drop {
                    Ok(_) => {printers::warn("API несподівано завершився".to_string())},
                    Err(e) => {
                        let msg = format!("API упав з помилкою : {e}");
                        printers::warn(msg);
                    }
                }
            }
            db_drop = &mut db_handler => {
                match db_drop {
                    Ok(_) => {printers::warn("Воркер DB упав без помилки".to_string())},
                    Err(e) => {
                        let msg = format!("Воркер DB упав з помилкою : {e}");
                        printers::warn(msg);
                    }
                }
            }
            data_master_drop = &mut data_master_handler => {
                match data_master_drop {
                    Ok(_) => {printers::warn("Контролер даних упав без помилки".to_string())},
                    Err(e) => {
                        let msg = format!("Контролер даних упав з помилкою : {e}");
                        printers::warn(msg);
                    }
                }
            }
            reader_drop = &mut reader_handler => {
                match reader_drop {
                    Ok(_) => {printers::warn("Опитувач упав без помилки".to_string())},
                    Err(e) => {
                        let msg = format!("Опитувач упав з помилкою : {e}");
                        printers::warn(msg);
                    }
                }
            }
        }
        // якшо сюда дійде то пиздець!!!
        reader_handler.abort();
        data_master_handler.abort();
        db_handler.abort();
        api_handler.abort();
        let stdin = tokio::io::stdin();
        let mut reader = TokioBufReader::new(stdin);
        let mut input = String::new();
        let _ = reader.read_line(&mut input).await;
    }
}
