use crate::reader::structs::modbus_measure::ModbusMeasure;

#[derive(Debug, Clone, Copy, Default)]
pub enum RegType {
    Coils,
    Discrete,
    Input,
    #[default]
    Holding,
}

impl RegType {
    pub fn check_type(reg_type: &str) -> RegType {
        match reg_type {
            "coils" => RegType::Coils,
            "discrete" => RegType::Discrete,
            "holding" => RegType::Holding,
            "input" => RegType::Input,
            _ => RegType::Coils
        }
    }
}

pub trait ValueInterface {
    fn init(&mut self, settings: String, id: i32, logging: bool) -> Vec<i32>;
    fn find_your_registers(&mut self, dataset: &[i32]) -> bool;
    fn get_value(&self, reg_list: &[u16], timestamp: i64) -> ModbusMeasure;
    fn fail(&self, timestamp: i64) -> ModbusMeasure;
    fn get_type(&self) -> RegType;

    fn get_id(&self) -> i32;
}