use chrono::{DateTime, Local};
use colored::*;
use serde::Deserialize;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TrySendError};
use std::sync::{Arc, OnceLock};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const FILE_QUEUE_SIZE: usize = 1024;
const CONSOLE_QUEUE_SIZE: usize = 256;
static LOGGER: OnceLock<Logger> = OnceLock::new();

#[derive(Deserialize)]
#[serde(default)]
struct LoggerConfig {
    console_enabled: bool,
}

impl Default for LoggerConfig {
    fn default() -> Self {
        Self {
            console_enabled: true,
        }
    }
}

#[derive(Default)]
struct Counters {
    file_dropped: AtomicU64,
    console_dropped: AtomicU64,
    file_errors: AtomicU64,
    console_errors: AtomicU64,
}

struct FileMessage {
    timestamp: DateTime<Local>,
    text: String,
}

enum ConsoleMessage {
    Line(String),
    Clear,
}

struct Logger {
    file_tx: SyncSender<FileMessage>,
    console_tx: Option<SyncSender<ConsoleMessage>>,
    counters: Arc<Counters>,
}

fn try_send<T>(tx: &SyncSender<T>, message: T, dropped: &AtomicU64) {
    match tx.try_send(message) {
        Ok(()) => {}
        Err(TrySendError::Full(_)) | Err(TrySendError::Disconnected(_)) => {
            dropped.fetch_add(1, Ordering::Relaxed);
        }
    }
}

impl Logger {
    fn start<F, W>(
        mut file_output: F,
        console_output: Option<W>,
        file_capacity: usize,
        console_capacity: usize,
    ) -> (Self, Vec<JoinHandle<()>>)
    where
        F: FnMut(FileMessage) -> io::Result<()> + Send + 'static,
        W: Write + Send + 'static,
    {
        let counters = Arc::new(Counters::default());
        let (file_tx, file_rx) = mpsc::sync_channel::<FileMessage>(file_capacity);
        let file_counters = counters.clone();
        let file_worker = thread::Builder::new().name("logger-file".into()).spawn(move || {
            let mut reported = [0; 4];
            while let Ok(mut message) = file_rx.recv() {
                let current = [
                    file_counters.file_dropped.load(Ordering::Relaxed),
                    file_counters.console_dropped.load(Ordering::Relaxed),
                    file_counters.file_errors.load(Ordering::Relaxed),
                    file_counters.console_errors.load(Ordering::Relaxed),
                ];
                if current != reported {
                    message.text = format!(
                        "[LOGGER] cumulative counters: file_dropped={}, console_dropped={}, file_errors={}, console_errors={}\n{}",
                        current[0], current[1], current[2], current[3], message.text,
                    );
                }
                if file_output(message).is_err() {
                    // Never report a file error through stdout or recursively through this queue.
                    file_counters.file_errors.fetch_add(1, Ordering::Relaxed);
                } else {
                    reported = current;
                }
            }
        }).expect("Cannot start file logger thread");
        let mut workers = vec![file_worker];
        let console_tx = match console_output {
            Some(output) => {
                let (tx, rx) = mpsc::sync_channel(console_capacity);
                let console_counters = counters.clone();
                let worker = thread::Builder::new()
                    .name("logger-console".into())
                    .spawn(move || {
                        console_loop(rx, output, &console_counters);
                    })
                    .expect("Cannot start console logger thread");
                workers.push(worker);
                Some(tx)
            }
            None => None,
        };
        (
            Self {
                file_tx,
                console_tx,
                counters,
            },
            workers,
        )
    }

    fn send(&self, timestamp: DateTime<Local>, file_text: String, console_text: String) {
        try_send(
            &self.file_tx,
            FileMessage {
                timestamp,
                text: file_text,
            },
            &self.counters.file_dropped,
        );
        if let Some(tx) = &self.console_tx {
            try_send(
                tx,
                ConsoleMessage::Line(console_text),
                &self.counters.console_dropped,
            );
        }
    }
}

fn console_loop<W: Write>(rx: Receiver<ConsoleMessage>, mut output: W, counters: &Counters) {
    let mut date = Local::now().date_naive();
    loop {
        let result = match rx.recv_timeout(Duration::from_secs(1)) {
            Ok(ConsoleMessage::Line(text)) => writeln!(output, "{}", text),
            Ok(ConsoleMessage::Clear) => write!(output, "\x1b[2J\x1b[1;1H"),
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Err(mpsc::RecvTimeoutError::Timeout) => Ok(()),
        };
        if result.is_err() {
            counters.console_errors.fetch_add(1, Ordering::Relaxed);
        }
        let now = Local::now().date_naive();
        if now != date {
            if write!(output, "\x1b[2J\x1b[1;1H").is_err() {
                counters.console_errors.fetch_add(1, Ordering::Relaxed);
            }
            date = now;
        }
    }
}

fn log_path(root: &Path, timestamp: DateTime<Local>) -> PathBuf {
    root.join(timestamp.format("%y-%m").to_string())
        .join(format!("{}.txt", timestamp.format("%d")))
}

fn write_log_to_file(root: &Path, message: FileMessage) -> io::Result<()> {
    // Route by the event's date, even if the queue is drained after midnight.
    let path = log_path(root, message.timestamp);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    file.write_all(message.text.as_bytes())
}

pub fn init_logger() {
    LOGGER.get_or_init(|| {
        let (config, warning) = match fs::read_to_string("configs/logger.toml") {
            Ok(text) => match toml::from_str::<LoggerConfig>(&text) {
                Ok(config) => (config, None),
                Err(error) => (
                    LoggerConfig::default(),
                    Some(format!("Invalid logger config: {}", error)),
                ),
            },
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                (LoggerConfig::default(), None)
            }
            Err(error) => (
                LoggerConfig::default(),
                Some(format!("Cannot read logger config: {}", error)),
            ),
        };
        let console = if config.console_enabled {
            Some(io::stdout())
        } else {
            None
        };
        let (logger, _workers) = Logger::start(
            |message| write_log_to_file(Path::new("logs"), message),
            console,
            FILE_QUEUE_SIZE,
            CONSOLE_QUEUE_SIZE,
        );
        // These dedicated threads live for the process lifetime, including subsystem restarts.
        if let Some(warning) = warning {
            logger.send(Local::now(), format!("[WARNING] {}\n", warning), warning);
        }
        logger
    });
}

pub fn clear_console() {
    let Some(logger) = LOGGER.get() else {
        return;
    };
    if let Some(tx) = &logger.console_tx {
        try_send(tx, ConsoleMessage::Clear, &logger.counters.console_dropped);
    }
}

pub fn err(msg: String) {
    send_log(msg, "[ERROR]".red().bold(), "ERROR");
}

pub fn warn(msg: String) {
    send_log(msg, "[WARNING]".yellow().bold(), "WARNING");
}

pub fn event(msg: String) {
    send_log(msg, "[EVENT]".blue().bold(), "EVENT");
}

pub fn debug(msg: String) {
    send_log(msg, "[DEBUG]".purple().bold(), "DEBUG");
}

fn send_log(msg: String, log_head: ColoredString, head: &str) {
    let now = Local::now();
    let ts = now.format("%y-%m-%d %H:%M:%S");
    if let Some(logger) = LOGGER.get() {
        logger.send(
            now,
            format!("[{}] : {} -> {}\n", head, ts, msg),
            format!("{} : {} -> {}", log_head, ts, msg),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    struct BlockedConsole {
        entered: Option<mpsc::Sender<()>>,
        release: Receiver<()>,
    }

    impl Write for BlockedConsole {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if let Some(entered) = self.entered.take() {
                entered.send(()).unwrap();
                self.release.recv().unwrap();
            }
            Ok(bytes.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[tokio::test(flavor = "current_thread")]
    async fn blocked_console_does_not_block_tokio_or_file_logging() {
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let (file_tx, file_rx) = mpsc::channel();
        let (logger, workers) = Logger::start(
            move |message| {
                file_tx.send(message.text).unwrap();
                Ok(())
            },
            Some(BlockedConsole {
                entered: Some(entered_tx),
                release: release_rx,
            }),
            1024,
            1,
        );
        logger.send(Local::now(), "first\n".into(), "first".into());
        entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        file_rx.recv_timeout(Duration::from_secs(2)).unwrap();

        // The console is still blocked. More log events must leave Tokio free
        // and continue to reach the independent file thread.
        for _ in 0..100 {
            logger.send(Local::now(), "next\n".into(), "next".into());
        }
        let heartbeat = tokio::spawn(async {
            tokio::time::sleep(Duration::from_millis(10)).await;
            42
        });
        let result = tokio::time::timeout(Duration::from_secs(2), heartbeat).await;
        let file_result = file_rx.recv_timeout(Duration::from_secs(2));
        let dropped = logger.counters.console_dropped.load(Ordering::Relaxed);
        release_tx.send(()).unwrap();
        drop(logger);
        for worker in workers {
            worker.join().unwrap();
        }
        assert_eq!(result.unwrap().unwrap(), 42);
        assert!(file_result.unwrap().contains("next"));
        assert_eq!(dropped, 99);
    }

    #[test]
    fn full_file_queue_does_not_block_producers() {
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel::<()>();
        let (logger, workers) = Logger::start(
            move |_| {
                if entered_tx.send(()).is_ok() {
                    let _ = release_rx.recv();
                }
                Ok(())
            },
            None::<io::Sink>,
            1,
            1,
        );
        logger.send(Local::now(), "first\n".into(), "first".into());
        entered_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        logger.send(Local::now(), "queued\n".into(), "queued".into());
        logger.send(Local::now(), "dropped\n".into(), "dropped".into());
        let dropped = logger.counters.file_dropped.load(Ordering::Relaxed);
        drop(release_tx); // Release every pending write before joining.
        drop(logger);
        for worker in workers {
            worker.join().unwrap();
        }
        assert_eq!(dropped, 1);
    }

    #[test]
    fn file_errors_are_counted_and_reported_after_recovery() {
        let (written_tx, written_rx) = mpsc::channel();
        let mut first = true;
        let (logger, workers) = Logger::start(
            move |message| {
                if first {
                    first = false;
                    return Err(io::Error::other("disk unavailable"));
                }
                written_tx.send(message.text).unwrap();
                Ok(())
            },
            None::<io::Sink>,
            8,
            1,
        );
        logger.send(Local::now(), "first\n".into(), String::new());
        logger.send(Local::now(), "second\n".into(), String::new());
        let written = written_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        drop(logger);
        for worker in workers {
            worker.join().unwrap();
        }
        assert!(written.contains("file_errors=1"));
        assert!(written.ends_with("second\n"));
    }

    #[test]
    fn file_rotation_uses_event_date_and_writes_to_disk() {
        let root = std::env::temp_dir().join(format!("logger-test-{}", uuid::Uuid::new_v4()));
        let before = Local
            .with_ymd_and_hms(2026, 8, 31, 23, 59, 59)
            .single()
            .unwrap();
        let after = Local
            .with_ymd_and_hms(2026, 9, 1, 0, 0, 1)
            .single()
            .unwrap();
        write_log_to_file(
            &root,
            FileMessage {
                timestamp: before,
                text: "August\n".into(),
            },
        )
        .unwrap();
        write_log_to_file(
            &root,
            FileMessage {
                timestamp: after,
                text: "September\n".into(),
            },
        )
        .unwrap();
        assert_eq!(
            fs::read_to_string(root.join("26-08/31.txt")).unwrap(),
            "August\n"
        );
        assert_eq!(
            fs::read_to_string(root.join("26-09/01.txt")).unwrap(),
            "September\n"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn logger_initialization_is_idempotent() {
        init_logger();
        let original = LOGGER.get().unwrap() as *const Logger;
        init_logger();
        assert_eq!(original, LOGGER.get().unwrap() as *const Logger);
    }
}
