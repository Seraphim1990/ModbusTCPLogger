use std::sync::Arc;
use crate::reader::reader_loop::decoding_plugins::value_interface::{RegType, ValueInterface};
use crate::reader::reader_loop::decoding_plugins::{
    coma_shift::ComaShift,
    bit_in_word::BitInWord,
    satec_double_register::SatecDoubleRegistersInt32
};
use crate::reader::reader_loop::decoding_plugins::bit_in_word::BitInWordSerde;
use crate::reader::reader_loop::decoding_plugins::coma_shift::ComaShiftSerde;
use crate::reader::reader_loop::decoding_plugins::satec_double_register::SatecDoubleRegistersSerde;
use crate::reader::structs::modbus_measure::ModbusMeasure;

pub fn get_plugin(id: i32) -> Result<RegisterDecodingPlugin, Arc<str>>{
    match id {
        1 => Ok(RegisterDecodingPlugin::Shift(ComaShift::default())),
        2 => Ok(RegisterDecodingPlugin::Satec(SatecDoubleRegistersInt32::default())),
        3 => Ok(RegisterDecodingPlugin::Bit(BitInWord::default())),
        _ => {
            Err(Arc::from("Невідомий тип декодування"))
        }
    }
}

pub enum RegisterDecodingPlugin {
    Bit(BitInWord),
    Satec(SatecDoubleRegistersInt32),
    Shift(ComaShift),
}

impl RegisterDecodingPlugin {
    pub fn init(&mut self, settings: String, id: i32, logging: bool) -> Vec<i32> {
        match self {
            RegisterDecodingPlugin::Bit(p) => {p.init(settings, id, logging)},
            RegisterDecodingPlugin::Satec(p) => {p.init(settings, id, logging)},
            RegisterDecodingPlugin::Shift(p) => {p.init(settings, id, logging)},
        }
    }

    pub fn find_your_registers(&mut self, dataset: &[i32]) -> bool {
        match self {
            RegisterDecodingPlugin::Bit(p) => {p.find_your_registers(dataset)},
            RegisterDecodingPlugin::Satec(p) => {p.find_your_registers(dataset)},
            RegisterDecodingPlugin::Shift(p) => {p.find_your_registers(dataset)},
        }
    }
    pub fn get_value(&self, reg_list: &[u16], timestamp: i64) -> ModbusMeasure {
        match self {
            RegisterDecodingPlugin::Bit(p) => {p.get_value(reg_list, timestamp)},
            RegisterDecodingPlugin::Satec(p) => {p.get_value(reg_list, timestamp)},
            RegisterDecodingPlugin::Shift(p) => {p.get_value(reg_list, timestamp)},
        }
    }

    pub fn get_type(&self) -> RegType {
        match self {
            RegisterDecodingPlugin::Bit(p) => {p.get_type()},
            RegisterDecodingPlugin::Satec(p) => {p.get_type()},
            RegisterDecodingPlugin::Shift(p) => {p.get_type()},
        }
    }
    pub fn fail(&self, timestamp: i64) -> ModbusMeasure {
        match self {
            RegisterDecodingPlugin::Bit(p) => {p.fail(timestamp)},
            RegisterDecodingPlugin::Satec(p) => {p.fail(timestamp)},
            RegisterDecodingPlugin::Shift(p) => {p.fail(timestamp)},
        }
    }

    pub fn get_id(&self) -> i32 {
        match self {
            RegisterDecodingPlugin::Bit(p) => {p.get_id()},
            RegisterDecodingPlugin::Satec(p) => {p.get_id()},
            RegisterDecodingPlugin::Shift(p) => {p.get_id()},
        }
    }
}


pub fn check_json(id: i32, json_data: &str) -> Result<(), String> {
    match id {
        1 => {
            let _: ComaShiftSerde = serde_json::from_str(json_data).map_err(|e|
                format!("Помилка json ComaShift: {}", e)
            )?;
        },
        2 => {
            let _: SatecDoubleRegistersSerde = serde_json::from_str(json_data).map_err(|e|
                format!("Помилка json SatecDoubleRegisters: {}", e)
            )?;
        },
        3 => {
            let _: BitInWordSerde = serde_json::from_str(json_data).map_err(|e|
                format!("Помилка json BitInWord: {}", e)
            )?;
        },
        _ => {
            return Err("Невідомий тип декодування".to_string());
        }
    }
    Ok(())
}