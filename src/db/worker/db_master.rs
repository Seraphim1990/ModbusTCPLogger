use tokio::sync::mpsc;
use crate::db::states::init_db;
use crate::messages::main_msg::MainMsg;
use crate::messages::requests::request_struct::Request;
use crate::db::worker::{
    node_worker::{node_get, command_node},
    device_worker::{devise_get, command_device},
    value_worker::{value_get, command_value},
    user_worker::{users_get, user_command},
    user_group_worker::{groups_get, group_command},
    user_sub_group_workers::{user_sub_group_command, user_sub_group_get},
    assign_worker::command_assign,
    tokens_worker::{get_token, update_token}
};
use crate::logger::printers;
use crate::messages::commands::command::CommandType;
use crate::messages::events::event::Event;

use crate::db::hasher::hash_master::measure_master;
use crate::messages::config_event::ConfigEvent;

pub async fn run_db_master(rx: mpsc::Receiver<MainMsg>, tx_to_reader: mpsc::Sender<ConfigEvent>) {
    printers::event(String::from("Старт воркера бази даних"));

    let mut rx = rx;
    let pool = init_db(20).await;

    let (measure_tx, measure_rx) = mpsc::channel::<MainMsg>(100);

    tokio::spawn(measure_master(pool.clone(), measure_rx));

    while let Some(msg) = rx.recv().await {
        match msg {
            MainMsg::Command(msg) => {
                match msg.cmd {
                    CommandType::NodeCommand(_) => command_node(&pool, msg, tx_to_reader.clone()),
                    CommandType::DeviceCommand(_) => command_device(&pool, msg, tx_to_reader.clone()),
                    CommandType::ValueCommand(_) => command_value(&pool, msg, tx_to_reader.clone()),
                    CommandType::UserCommand(_) => user_command(&pool, msg),
                    CommandType::AssignGroupsAndValuesCommand(_) => command_assign(&pool, msg),
                    CommandType::GroupCommand(_) => group_command(&pool, msg),
                    CommandType::SubGroupCommand(_) => user_sub_group_command(&pool, msg),
                    CommandType::TokenUpdate(_) => update_token(&pool, msg)
                }
            },
            MainMsg::Request(msg) => {
                match msg {
                    Request::GetNode(request) =>  node_get(&pool, request),
                    Request::GetDevice(request) => devise_get(&pool, request),
                    Request::GetValue(request) => value_get(&pool, request),
                    Request::GetUser(user) => users_get(&pool, user),
                    Request::GetGroup(group) => groups_get(&pool, group),
                    Request::GetSubGroup(sub_group) => user_sub_group_get(&pool, sub_group),
                    Request::GetToken(token) => get_token(&pool, token),
                    Request::GetMeasure(measure) => {
                        let req = MainMsg::Request(Request::GetMeasure(measure));
                        if let Err(e) = measure_tx.send(req).await {
                            printers::err(format!("Помилка відправки події для зберігання: {}", e));
                        }
                    },
                }
            },
            MainMsg::Event(event) => {
                match event {
                    Event::DeviceEvent(dev_ev) => {
                        let ev = MainMsg::Event(Event::DeviceEvent(dev_ev));
                        if let Err(e) = measure_tx.send(ev).await {
                            printers::err(format!("Помилка відправки події для зберігання: {}", e));
                        }
                    },
                    Event::NodeEvent(_) => {}
                }
            }
        }
    }
    printers::warn(String::from("Воркер бази даних упав!!!!"));
}
