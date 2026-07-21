use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;
use tokio::sync::mpsc;
use crate::messages::main_msg::MainMsg;
use crate::api::web_sockets::live_socket_unit::{CoordUnitWebSocketCommand, CoordUnitWebSocketData};
use crate::logger::printers;
use crate::messages::events::device_event::DeviceEventType;
use crate::messages::events::event::Event;
use crate::messages::events::node_event::NodeEventType;

pub async fn web_sock_coord(mut from_reader: mpsc::Receiver<MainMsg>, mut from_api: mpsc::Receiver<CoordUnitWebSocketCommand>) {

    let mut nodes: HashMap<i32, NodeEventType> = HashMap::new();
    let mut devises: HashMap<i32, DeviceEventType> = HashMap::new();
    let mut values: HashMap<Arc<String>, f64> = HashMap::new();

    let mut subscribers: HashMap<usize, CoordUnitWebSocketData> = HashMap::new();

    loop {
        tokio::select! {
            from_reader_msg = from_reader.recv() => {
                match from_reader_msg {
                    Some(msg) => {
                        if let MainMsg::Event(event) = msg {
                            match event {
                                Event::NodeEvent(event) => {
                                    if check_change(&mut nodes, event.id, event.event.clone()) {
                                        for unit in subscribers.values_mut() {
                                            unit.node_events(event.id, &event.event)
                                        }
                                        flush_subscribers(&mut subscribers);
                                    }
                                },
                                Event::DeviceEvent(event) => {
                                    if check_change(&mut devises, event.id, event.event.clone()) {
                                        for unit in subscribers.values_mut() {
                                            unit.device_events(event.id, &event.event)
                                        }
                                    }
                                    for value in &event.measures {
                                        if check_change(&mut values, value.tag.clone(), value.measure_value) {
                                            for unit in subscribers.values_mut() {
                                                unit.value_events(value.tag.clone(), value.measure_value)
                                            }
                                        }
                                    }
                                    flush_subscribers(&mut subscribers);
                                },
                            }
                        } else {
                            printers::err("web_sock_coord не повинен приймати нічого крім MainMsg::Event".to_string());
                        }
                    },
                    None => {
                        printers::warn("Падіння каналу від головного контролера в маршрутизаторі вебсокетів".to_string());
                        panic!() // головний контролер упав, він має працювати завжди
                    }
                }
            }
            from_api_msg = from_api.recv() => {
                match from_api_msg {
                    Some(subscriber) => {
                        match subscriber {
                            CoordUnitWebSocketCommand::New{id, mut unit} => {
                                if first_write(&mut unit, &mut nodes, &mut devises, &mut values) {
                                    subscribers.insert(id, unit);
                                }
                            },
                            CoordUnitWebSocketCommand::Change{old_id, new_id, mut unit} => {
                                subscribers.remove(&old_id);
                                if first_write(&mut unit, &mut nodes, &mut devises, &mut values) {
                                    subscribers.insert(new_id, unit);
                                }
                            },
                            CoordUnitWebSocketCommand::Delete{id} => {
                                subscribers.remove(&id);
                            }
                        }
                    },
                    None => {
                        printers::warn("Падіння каналу від API в маршрутизаторі вебсокетів".to_string());
                    }
                }
            }
        }
    }
}

fn first_write(subscriber: &mut CoordUnitWebSocketData,
                     nodes: &mut HashMap<i32, NodeEventType>,
                     devises: &mut HashMap<i32, DeviceEventType>,
                     values: &mut HashMap<Arc<String>, f64>) -> bool
{
    for (key, val) in nodes.iter() {
        subscriber.node_events(*key, val);
    }
    for (key, val) in devises.iter() {
        subscriber.device_events(*key, val);
    }
    for (key, val) in values.iter() {
        subscriber.value_events(key.clone(), *val);
    }
    subscriber.flush()
}

fn check_change<K, V>(target: &mut HashMap<K, V>, key: K, value: V) -> bool
where
    K: Eq + Hash,
    V: PartialEq,
{
    match target.entry(key) {
        std::collections::hash_map::Entry::Occupied(mut entry) => {
            if *entry.get() == value {
                false
            } else {
                entry.insert(value);
                true
            }
        }
        std::collections::hash_map::Entry::Vacant(entry) => {
            entry.insert(value);
            true
        }
    }
}

fn flush_subscribers(subscribers: &mut HashMap<usize, CoordUnitWebSocketData>) {
    subscribers.retain(|_, sub| sub.flush());
}