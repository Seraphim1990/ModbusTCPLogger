use std::sync::Arc;
use crate::reader::reader_loop::decoding_plugins::value_interface::{ValueInterface, RegType};
use crate::reader::structs::modbus_measure::ModbusMeasure;

use serde::{Deserialize, Serialize};
use crate::logger::printers;

#[derive(Serialize, Deserialize)]
pub struct BitInWordSerde {
    tag: String,
    addr: i32,
    #[serde(rename = "bitAddr")]
    bit_addr: i32,
    #[serde(rename = "regType")]
    reg_type: String,
}

#[derive(Default)]
pub struct BitInWord {
    addr: i32,
    tag: Arc<String>,
    pos_in_list: usize,
    bit_addr: i32,
    id: i32,
    is_logging: bool,
    is_init: bool,
    m_type: RegType,
}
impl ValueInterface for BitInWord {
    fn init(&mut self, settings: String, id: i32, logging: bool) -> Vec<i32> {

        let setting = serde_json::from_str(settings.as_str());

        let init_data: BitInWordSerde = match setting {
            Ok(s) => s,
            Err(e) => {
                let msg = format!("Помилкка десеріалізації плагіну BitInWord: \n{:?}\n{settings}", e);
                printers::err(msg);
                return Vec::new();
            }
        };
        self.id = id;
        self.tag = Arc::new(init_data.tag);
        self.addr = init_data.addr;
        self.bit_addr = init_data.bit_addr;
        self.is_logging = logging;
        self.m_type = RegType::check_type(init_data.reg_type.as_str());

        vec![self.addr]
    }
    fn find_your_registers(&mut self, dataset: &[i32]) -> bool {
        if self.is_init {return false};
        for (i, &value) in dataset.iter().enumerate() {
            if value == self.addr {
                self.is_init = true;
                self.pos_in_list = i;
                return true;
            }
        }
        false
    }

    fn get_id(&self) -> i32 {
        self.id
    }
    fn get_value(&self, reg_list: &[u16], timestamp: i64) -> ModbusMeasure {
        if reg_list.len() <= self.pos_in_list { // заглушка для тестування, це ж довбаний раст
            panic!("Паніка при декодуванні значення {}, вихід за межі вектору", &self.tag);
        }

        let measure = reg_list[self.pos_in_list] >> self.bit_addr as u32 & 0x01;
        ModbusMeasure {
            measure_value: measure as f64,
            is_logging: self.is_logging,
            value_id: self.id as u32,
            tag: self.tag.clone(),
            measure_time: timestamp
        }
    }
    fn get_type(&self) -> RegType {
        self.m_type
    }

    fn fail(&self, timestamp: i64) -> ModbusMeasure {
        ModbusMeasure {
            measure_value: f64::MIN,
            is_logging: self.is_logging,
            value_id: self.id as u32,
            tag: self.tag.clone(),
            measure_time: timestamp
        }
    }
}