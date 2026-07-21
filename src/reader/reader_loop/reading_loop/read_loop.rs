use tokio::sync::mpsc;
use crate::messages::requests::node_request::{GetAllNodes, GetByIp, NodeRequest};
use tokio::sync::oneshot;
use crate::db::schemas::node::{NodeRead};
use crate::logger::printers;
use tokio::sync::broadcast;
use std::time::Duration;
use sqlx::__rt::sleep;
use crate::messages::{
    main_msg::MainMsg,
    requests::request_struct::Request,
};
use crate::messages::config_event::{ConfigEvent, ConfigEventType};
use crate::reader::reader_loop::reading_loop::read_master::node_loop;

pub async fn node_master(from_controller: mpsc::Receiver<ConfigEvent>, to_controller: mpsc::Sender<MainMsg>,) {
    printers::event(String::from("Старт опитувача"));
    let mut from_controller = from_controller;

    let (to_node_tx, mut from_node_rx, from_node_tx) = init_nodes(&to_controller).await;

    loop {
        tokio::select! {
            controller_msg = from_controller.recv() => {
                match controller_msg {
                    Some(msg) => {
                        match &msg.event_type {
                            ConfigEventType::Create => {
                                create_new_node(&to_node_tx, &from_node_tx, &to_controller, &msg.data).await;
                            },
                            ConfigEventType::Update | ConfigEventType::Delete => {
                                send_to_workers(&to_node_tx, msg)
                            },
                        }
                    }
                    None => {
                        printers::warn(String::from("Падіння каналу від головного контролера до контролера нод"));
                        break;
                    }
                }
            }
            node_msg = from_node_rx.recv() => {
                match node_msg {
                    Some(msg) => {
                        match msg {
                            MainMsg::Event(event) => {
                                let send_res = to_controller.send(MainMsg::Event(event)).await;
                                match send_res {
                                    Ok(_) => {},
                                    Err(e) => {
                                        let msg = format!("Помилка відправки запиту від контролера нод до головного контролера\n {}", e);
                                        printers::err(msg);
                                    }
                                }
                            },
                            MainMsg::Request(request) => {
                                let send_res = to_controller.send(MainMsg::Request(request)).await;
                                match send_res {
                                    Ok(_) => {},
                                    Err(e) => {
                                        let msg = format!("Помилка відправки запиту від контролера нод до головного контролера\n {}", e);
                                        printers::err(msg);
                                    }
                                }
                            },
                            MainMsg::Command(_) => {
                                printers::err(String::from("Ноди не мають відправляти команди"));
                            },
                        }
                    }
                    None => {
                        printers::warn(String::from("Падіння каналу від нод до контролера нод"));
                        break;
                    }
                }
            }
        }
    }
}

async fn create_new_node(to_node_tx: &broadcast::Sender<ConfigEvent>,
                         from_node_tx: &mpsc::Sender<MainMsg>,
                         to_controller: &mpsc::Sender<MainMsg>,
                         node: &NodeRead) {
    let from_node_tx_clone = from_node_tx.clone();
    let node_subscribe = to_node_tx.subscribe();
    let mut try_counter = 0;
    loop {
        sleep(Duration::from_millis(50)).await;
        let (tx, rx) = oneshot::channel::<Result<Option<NodeRead>, ()>>();
        let node_msg = MainMsg::Request(
            Request::GetNode(
                NodeRequest::GetByIp(
                    GetByIp {
                        node_ip: node.ip.clone(),
                        request_channel: tx,
                    }
                )
            )
        );

        if let Err(e) = to_controller.send(node_msg).await {
            printers::err(format!("Помилка створення ноди через команду: {}", e));
        }

        if let Ok(Ok(Some(node_read))) = rx.await {
            tokio::spawn(async move {
                node_loop(from_node_tx_clone, node_subscribe, node_read).await;
            });
            break;
        }
        try_counter += 1;
        if try_counter == 5 {
            printers::err(String::from("Не вдалося створити ноду після 5 спроб"));
            break;
        }
    }
}

fn send_to_workers(tx: &broadcast::Sender<ConfigEvent>, signal: ConfigEvent) {
    let send_res = tx.send(signal);
    match send_res {
        Ok(_) => {},
        Err(e) => {
            let msg = format!("Помилка відправки запиту від контролера нод до головного контролера\n {}", e);
            printers::err(msg);
        }
    }
}

async fn init_nodes(to_controller: &mpsc::Sender<MainMsg>) -> (broadcast::Sender<ConfigEvent>, mpsc::Receiver<MainMsg>, mpsc::Sender<MainMsg>)
{
    let (rx, get_all_node_msg) = get_all_node_request();
    let _ = to_controller.send(get_all_node_msg).await;

    // Присвоюємо результат виконання match прямо в змінні
    let (to_node_tx, from_node_rx, from_node_tx) = match rx.await {
        Ok(Ok(nodes)) => {

            // Створюємо канали та мапу
            let (from_node_tx, from_node_rx) = mpsc::channel::<MainMsg>(100);
            let (to_node_tx, _to_node_rx) = broadcast::channel::<ConfigEvent>(100);

            for node in nodes {
                let from_node_tx_clone = from_node_tx.clone();
                let node_subscribe = to_node_tx.subscribe();

                tokio::spawn(async move {
                    node_loop(from_node_tx_clone, node_subscribe, node).await;
                });
            }

            // Повертаємо цей кортеж з match, якщо все успішно
            (to_node_tx, from_node_rx, from_node_tx)
        }
        Ok(Err(_)) => {
            printers::err(String::from("БД повернула помилку при отриманні нод в init_nodes"));
            panic!("Критична помилка ініціалізації нод");
        }
        Err(_) => {
            printers::err(String::from("Помилка отримання відповіді від БД (oneshot closed) в node_master"));
            panic!("Критична помилка зв'язку з БД на старті");
        }
    }; // <- крапка з комою обов'язкова, бо це присвоєння!

    // Тепер компілятор на 100% впевнений, що змінні ініціалізовані
    (to_node_tx, from_node_rx, from_node_tx)
}

fn get_all_node_request() -> (oneshot::Receiver<Result<Vec<NodeRead>, ()>>, MainMsg) {
    let (tx, rx) = oneshot::channel::<Result<Vec<NodeRead>, ()>>();

    let node_msg = MainMsg::Request(
        Request::GetNode(
            NodeRequest::GetAll(
                GetAllNodes {
                    request_channel: tx,
                }
            )
        )
    );
    (rx, node_msg)
}