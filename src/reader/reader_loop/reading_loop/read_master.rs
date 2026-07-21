// read_master.rs
use tokio::time::{sleep, Duration};
use tokio::sync::mpsc;
use crate::messages::{
    main_msg::MainMsg,
};
use tokio::sync::broadcast;
use crate::db::schemas::{
    node::NodeRead,
};
use crate::logger::printers;
use crate::reader::reader_loop::reading_loop::read_master_struct::ReadMaster;
use crate::messages::config_event::{ConfigEvent, ConfigEventType};

pub async fn node_loop(to_controller: mpsc::Sender<MainMsg>, from_controller: broadcast::Receiver<ConfigEvent>, conf: NodeRead) {
    let to_controller = to_controller;
    let mut from_controller = from_controller;
    let mut read_master = ReadMaster::new(&conf, to_controller);

    let mut sleep_time = 0;

    loop {
            tokio::select! {
                msg = from_controller.recv() => {
                    match msg {
                        Ok(msg) => {
                            match msg.event_type {
                                ConfigEventType::Update => {
                                    if msg.data.id == read_master.id(){
                                        read_master.update(msg.data).await;
                                    }
                                }
                                ConfigEventType::Delete => {
                                    if msg.data.id == read_master.id() {
                                        printers::warn(format!("Закінчення роботи ноди ip: {}", read_master.ip()));
                                        return;
                                    }
                                },
                                ConfigEventType::Create => {}
                            }
                        }
                        Err(e) => {
                            let msg = format!("Помилка каналу від контроллера до циклу оритуванні: \n{}", e);
                            printers::err(msg);
                        }
                    }
                },
                _ = sleep(Duration::from_millis(sleep_time)) => {
                    read_master.tick().await;
                }
            }
        sleep_time = read_master.when_next().max(20); // для чистки стану контексту
    }
}