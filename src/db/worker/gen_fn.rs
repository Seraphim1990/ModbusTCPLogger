use std::time::Duration;
use tokio::sync::oneshot::Sender;
use tokio::time::timeout;
use crate::logger::printers;

pub async fn run_db_with_timeout<F, T>(f: F, time_duration: u64, chan: Sender<T>, msg: &str)
where
    F: Future<Output = T>,
    T: Send + 'static,
{
    match timeout(Duration::from_secs(time_duration), f).await {
        Ok(res) => {
            if chan.send(res).is_err() {
                printers::err(format!("Помилка повернення калбеку {msg}"));
            }
        }
        Err(_) => {
            printers::warn(format!("Таймаут {msg}"));
        }
    }
}