use tokio::sync::mpsc;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::sync::Arc;
use crate::logger::printers;
use crate::messages::events::device_event::DeviceEventType;
use crate::messages::events::node_event::NodeEventType;

#[derive(Serialize)]
struct FlushPayload<'a> {
    #[serde(skip_serializing_if = "Vec::is_empty")]
    nodes: &'a Vec<NodeChange>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    devices: &'a Vec<DeviceChange>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    values: &'a Vec<ValueChange>,
}

#[derive(Serialize)]
struct NodeChange{
    id: i32,
    state: NodeEventType
}
#[derive(Serialize)]
struct DeviceChange{
    id: i32,
    state: DeviceEventType
}
#[derive(Serialize)]
struct ValueChange{
    tag: Arc<String>,
    value: f64,
}

#[derive(Deserialize, Default)]
struct SubscribeRequest {
    #[serde(default)]
    values: Vec<String>,
    #[serde(default)]
    nodes: Vec<i32>,
    #[serde(default)]
    devices: Vec<i32>,
}

pub enum CoordUnitWebSocketCommand {
    New {
        id: usize,
        unit: CoordUnitWebSocketData
    },
    Change{
        old_id: usize,
        new_id: usize,
        unit: CoordUnitWebSocketData,
    },
    Delete{
        id: usize,
    }
}

pub struct CoordUnitWebSocketData {
    tags: HashSet<String>,
    devises: HashSet<i32>,
    nodes: HashSet<i32>,
    send_chanel: mpsc::Sender<String>,
    nodes_change: Vec<NodeChange>,
    devise_change: Vec<DeviceChange>,
    value_change: Vec<ValueChange>,
}

impl CoordUnitWebSocketData {
    /*
    {
        "values": ["tag_1", "tag_2"],
        "nodes": [1, 15, 25],
        "devices": [17, 24]
    }
     */
    pub fn new(raw: &str, send_chanel: mpsc::Sender<String>) -> Result<Self, serde_json::Error> {
        let req: SubscribeRequest = serde_json::from_str(raw)?;
        Ok(CoordUnitWebSocketData {
            tags: req.values.into_iter().collect(),
            devises: req.devices.into_iter().collect(),
            nodes: req.nodes.into_iter().collect(),
            send_chanel,
            nodes_change: Vec::with_capacity(10),
            devise_change: Vec::with_capacity(10),
            value_change: Vec::with_capacity(10),
        })
    }
    pub fn node_events(&mut self, id: i32, state: &NodeEventType) {
        if self.nodes.contains(&id) {
            let node_ev = NodeChange{ id, state: state.clone() };
            self.nodes_change.push(node_ev);
        }
    }
    pub fn device_events(&mut self, id: i32, state: &DeviceEventType) {
        if self.devises.contains(&id) {
            let device_ev = DeviceChange{ id, state: state.clone() };
            self.devise_change.push(device_ev);
        }
    }
    pub fn value_events(&mut self, tag: Arc<String> , value: f64) {
        if self.tags.contains(tag.as_str()) {
            let value_ev = ValueChange{ tag: tag.clone(), value};
            self.value_change.push(value_ev);
        }
    }

    pub fn flush(&mut self) -> bool {
        if self.nodes_change.is_empty()
            && self.devise_change.is_empty()
            && self.value_change.is_empty() {
            return true; // нічого слати, юніт живий
        }

        let payload = FlushPayload {
            nodes: &self.nodes_change,
            devices: &self.devise_change,
            values: &self.value_change,
        };

        let result = match serde_json::to_string(&payload) {
            Ok(json) => self.send_chanel.try_send(json).is_ok(),
            Err(e) => {
                printers::event(format!("Помилка серіалізації flush: {}", e));
                true // помилка серіалізації — не привід вбивати юніт
            }
        };

        self.nodes_change.clear();
        self.devise_change.clear();
        self.value_change.clear();

        result
    }
}