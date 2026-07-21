use std::time::{SystemTime, UNIX_EPOCH};
use crate::reader::reader_loop::decoding_plugins::plugin_loader::RegisterDecodingPlugin;
use crate::reader::reader_loop::decoding_plugins::value_interface::RegType;
use crate::reader::structs::modbus_measure::ModbusMeasure;
use crate::logger::printers;
use std::sync::atomic::{AtomicBool, Ordering};

pub struct ReadStep{
    values: Vec<RegisterDecodingPlugin>,
    start: u16,
    length: u16,
    reg_type: RegType,
    broken: AtomicBool,
}

impl ReadStep{
    pub fn new(start: i32, length: i32, reg_type: RegType)->Self{
        ReadStep{
            values: Vec::new(),
            start: start as u16,
            length: length as u16,
            reg_type,
            broken: AtomicBool::new(false),
        }
    }
    pub fn get_type(&self)->RegType{
        self.reg_type
    }
    pub fn get_start(&self)->u16{
        self.start
    }
    pub fn get_length(&self)->u16{
        self.length
    }

    pub fn get_values_id(&self)->Vec<i32>{
        self.values.iter().map(|plugin|plugin.get_id()).collect()
    }
    pub fn add_unit(&mut self, unit: RegisterDecodingPlugin){
        self.values.push(unit);
    }
    pub fn get_value(&self, reg_list: &[u16]) -> Vec<ModbusMeasure>{
        let sys_time = SystemTime::now()
            .duration_since(UNIX_EPOCH);
        match sys_time {
            Ok(duration) => {
                let ts = duration.as_secs() as i64;
                self.values.iter()
                    .map( |value| value.get_value(reg_list, ts))
                    .collect::<Vec<ModbusMeasure>>()
            }
            Err(_) => {
                printers::warn(String::from("Помилка отримання часової відмітки"));
                Vec::new()
            }
        }
    }
    pub fn fail(&self) -> Vec<ModbusMeasure>{

        let sys_time = SystemTime::now()
            .duration_since(UNIX_EPOCH);
        match sys_time {
            Ok(duration) => {
                let ts = duration.as_secs() as i64;
                self.values.iter()
                    .map( |value| value.fail(ts))
                    .collect::<Vec<ModbusMeasure>>()
            }
            Err(_) => {
                printers::warn(String::from("Помилка отримання часової відмітки"));
                Vec::new()
            }
        }
    }
    pub fn mark_broken(&self) {
        self.broken.store(true, Ordering::Relaxed);
    }

    pub fn is_broken(&self) -> bool {
        self.broken.load(Ordering::Relaxed)
    }
}