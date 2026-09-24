use colored::*;
use chrono::{Days, Local};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;

use tokio::sync::mpsc;
use tokio::time::sleep;
use tokio::task::{spawn_blocking, JoinHandle};

static LOG_TX: OnceLock<mpsc::Sender<(String, String)>> = OnceLock::new();

pub fn init_logger() -> JoinHandle<()> {
    let (tx, rx) = mpsc::channel(20);
    LOG_TX.set(tx).ok();

    tokio::spawn(async move {
        let mut rx = rx;
        let mut str_buf = String::new();

        loop {
            let now = Local::now();
            let next_midnight = now
                .date_naive()
                .checked_add_days(Days::new(1))
                .unwrap()
                .and_hms_opt(0, 0, 0)
                .unwrap();
            let duration = (next_midnight - now.naive_local()).to_std().unwrap();

            tokio::select! {
            msg = rx.recv() => {
                match msg {
                    Some((console_msg, file_msg)) => {
                        str_buf += &file_msg;
                        println!("{}", &console_msg);
                        while let Ok((console_msg, file_msg)) = rx.try_recv() {
                            str_buf += &file_msg;
                            println!("{}", &console_msg);
                        }
                        write_log_to_file(&mut str_buf);  // TODO додати спавнблокінг та ваншот для калбеку!
                    }
                    None => break,
                }
            }
            _ = sleep(duration) => {
                print!("{esc}[2J{esc}[1;1H", esc = 27 as char);
                // і одразу flush буфера якщо є що писати
                if !str_buf.is_empty() {
                    write_log_to_file(&mut str_buf);
                    str_buf.clear();
                }
            }
        }
        }
    })
}
fn write_log_to_file(str_buf: &mut String) {
    let now = Local::now();

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
                    if let Err(e) = writeln!(f, "{}", str_buf) {
                        println!("🔥 Помилка збереження логу:\n{}", e);
                    } else {
                        str_buf.clear();
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
}

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
    send_log(msg, head,  "DEBUG");
}

fn send_log(msg: String, log_head: ColoredString, head: &str) {
    let now = Local::now();
    let ts = now.format("%y-%m-%d %H:%M:%S").to_string();
    let console_msg = format!("{} : {} -> {}", log_head, ts, msg);
    let file_msg = format!("[{}] : {} -> {}\n", head, ts, msg);

    if let Some(tx) = LOG_TX.get() {
        let _ = tx.try_send((console_msg, file_msg));
    }
}


