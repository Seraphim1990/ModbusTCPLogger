use std::sync::Arc;
use serde::{Deserialize, Serialize};
use crate::reader::reader_loop::decoding_plugins::value_interface::{ValueInterface, RegType};
use crate::reader::structs::modbus_measure::ModbusMeasure;

use crate::logger::printers;

#[derive(Serialize, Deserialize)]
pub struct SatecDoubleRegistersSerde {
    pub tag: String,
    #[serde(rename = "regType")]
    pub reg_type: String,
    #[serde(rename = "isSigned")]
    pub is_signed: bool,
    #[serde(rename = "hiRegister")]
    pub hi_register: i32,
    #[serde(rename = "loRegister")]
    pub lo_register: i32,
    pub multiplier: i32,
}

#[derive(Default)]
pub struct SatecDoubleRegistersInt32 {
    hi_register: i32,
    lo_register: i32,
    hi_pos_register: i32,
    lo_pos_register: i32,
    multiplier: f64,
    id: i32,
    is_logging: bool,
    is_signed: bool,
    is_init: bool,
    m_type: RegType,
    tag: Arc<String>,
}
impl ValueInterface for SatecDoubleRegistersInt32 {
    fn init(&mut self, settings: String, id: i32, logging: bool) -> Vec<i32> {
        let setting = serde_json::from_str(settings.as_str());
        let init_data: SatecDoubleRegistersSerde = match setting {
            Ok(s) =>  s,
            Err(e) => {
                let msg = format!("Помилкка десеріалізації плагіну SatecDoubleRegistersInt32: \n{:?}\n{settings}", e);
                printers::err(msg);
                return Vec::new();
            }
        };
        self.id = id;
        self.tag = Arc::new(init_data.tag);
        self.hi_register = init_data.hi_register;
        self.lo_register = init_data.lo_register;
        self.multiplier = 10.0_f64.powf(init_data.multiplier as f64);
        self.is_logging = logging;
        self.is_signed = init_data.is_signed;
        self.m_type = RegType::check_type(init_data.reg_type.as_str());

        vec![self.hi_register, self.lo_register]
    }
    fn get_id(&self) -> i32 {
        self.id
    }
    fn find_your_registers(&mut self, dataset: &[i32]) -> bool {
        if self.is_init {return false};

        let mut hi_found = false;
        let mut lo_found = false;
        for (i, &value) in dataset.iter().enumerate() {
            if self.hi_register == value{
                self.hi_pos_register = i as i32;
                hi_found = true;
            }
            if self.lo_register == value{
                self.lo_pos_register = i as i32;
                lo_found = true;
            }
        }

        if hi_found && lo_found {
            self.is_init = true;
            return true;
        }
        false
    }
    fn get_value(&self, reg_list: &[u16], timestamp: i64) -> ModbusMeasure {

        if reg_list.len() <= self.hi_pos_register as usize || reg_list.len() <= self.lo_pos_register as usize { // заглушка для тестування, це ж довбаний раст
            panic!("Паніка при декодуванні значення {}, вихід за межі вектору", &self.tag);
        }
        let measure;
        let mut hi_data = reg_list[self.hi_pos_register as usize] as f64;
        let lo_data = reg_list[self.lo_pos_register as usize] as f64;
        if self.is_signed {
            if hi_data > 32767.0 { hi_data = -hi_data }
            measure = (hi_data + lo_data) / self.multiplier;
        } else {
            measure = ((hi_data * 65536.0) + lo_data) / self.multiplier;
        }

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