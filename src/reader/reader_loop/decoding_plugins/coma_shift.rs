use std::sync::Arc;
use crate::reader::reader_loop::decoding_plugins::value_interface::{ValueInterface, RegType};
use crate::reader::structs::modbus_measure::ModbusMeasure;
use serde::{Deserialize, Serialize};
use crate::logger::printers;
/*
'{"tag": "water_bast_level",
"addr": 1,
"regType": "holding",
"isLoging": false,
"multiplier": 2}'
 */
#[derive(Serialize, Deserialize)]
pub struct ComaShiftSerde {
    pub tag: String,
    pub addr: i32,
    #[serde(rename = "regType")]
    pub reg_type: String,
    pub multiplier: i32,
}


#[derive(Default)]
pub struct ComaShift {
    id: i32,
    tag: Arc<String>,
    addr: i32,
    multiplier: f64,
    m_type: RegType,
    is_logging: bool,
    is_init: bool,
    pos_in_list: usize,
}
impl ValueInterface for ComaShift {
    fn init(&mut self, settings: String, id: i32, logging: bool) -> Vec<i32> {
        let setting = serde_json::from_str(settings.as_str());

        let init_data: ComaShiftSerde = match setting {
            Ok(s) => s,
            Err(e) => {
                let msg = format!("Помилкка десеріалізації плагіну ComaShift: \n{:?}\n{settings}", e);
                printers::err(msg);
                return Vec::new();
            }
        };

        self.id = id;
        self.tag = Arc::new(init_data.tag);
        self.addr = init_data.addr;
        self.multiplier = 10.0_f64.powf(init_data.multiplier as f64);  // 10.0_f64 це літерал <-------------
        self.m_type = RegType::check_type(init_data.reg_type.as_str());
        self.is_logging = logging;

        vec![self.addr]
    }
    fn find_your_registers(&mut self, dataset: &[i32]) -> bool {
        if self.is_init {return false};
        for (i, &item) in dataset.iter().enumerate() {
            if item == self.addr {
                self.pos_in_list = i;
                self.is_init = true;
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
        let res = reg_list[self.pos_in_list] as i16;
        let measure = res as f64 / self.multiplier;

        ModbusMeasure {
            measure_value: measure,
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