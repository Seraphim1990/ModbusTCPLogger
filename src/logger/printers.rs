use colored::*;
use chrono::Local;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;

pub fn err(msg: String) {
    let head = "[ERROR]".red().bold();
    send_log(msg, head, "ERROR");
}

pub fn warn(msg: String) {
    let head = "[WARNING]".yellow().bold();
    send_log(msg, head, "WARNING");
}
pub fn event(msg: String) {
    let head = "[EVENT]".blue().bold();
    send_log(msg, head,  "EVENT");
}

pub fn debug(msg: String) {
    let head = "[DEBUG]".purple().bold();
    send_log(msg, head,  "EVENT");
}

fn send_log(msg: String, log_head: ColoredString, head: &str) {
    let now = Local::now();
    let ts = now.format("%y-%m-%d %H:%M:%S").to_string();
    let console_msg = format!("{} : {} -> {}", log_head, ts, msg);
    let file_msg = format!("[{}] : {} -> {}\n", head, ts, msg);
    println!("{}", &console_msg);

    tokio::task::spawn_blocking(move || {
        let base_dir = PathBuf::from("logs");

        let month_dir = base_dir.join(now.format("%y-%m").to_string());

        let file_path = month_dir.join(format!("{}.txt", now.format("%d")));

        let dirs = fs::create_dir_all(&month_dir);

        match dirs {
            Ok(_) => {
                let file = OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&file_path);

                match file {
                    Ok(mut f) => {
                        if let Err(e) = writeln!(f, "{}", file_msg) {
                            println!("🔥 Помилка збереження логу:\n{}", e);
                        }
                    },
                    Err(e) => {
                        println!("🔥 Помилка відкриття файлу логу:\n{}", e);
                    }
                };
            },
            Err(e) => {
                println!("🔥 Помилка створення\\відкриття теки логу:\n{}", e);
            }
        }

    });
}


