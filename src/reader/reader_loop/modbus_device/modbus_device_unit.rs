use std::time::{SystemTime, UNIX_EPOCH};

use crate::db::schemas::device::DeviceRead;
use crate::db::schemas::value_unit::ValueRead;
use crate::reader::reader_loop::modbus_device::pool_iterator::{PollValueIterator, PoolValueRefIterator};


pub struct ModbusDeviceUnit {
    id: i32,
    _name: String,
    address: i32,
    time_for_recall: i32,
    timeout: u64,
    retry_count: i32,
    is_active: bool,
    read_by_group: bool,
    next_read_at: u64,
    value_pool: PollValueIterator,
}

impl ModbusDeviceUnit {
    pub fn new(setting: DeviceRead) -> ModbusDeviceUnit {

        let time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")  // упаде, то й похуй, це ж раст...
            .as_millis() as u64;

        ModbusDeviceUnit {
            id: setting.id,
            _name: setting.device_name.unwrap_or_default(),
            address: setting.address,
            timeout: setting.timeout as u64,
            time_for_recall: setting.time_for_recall,
            retry_count: setting.retry_count,
            is_active: setting.is_active,
            read_by_group: setting.read_by_group,
            next_read_at: time,
            value_pool: PollValueIterator::new()
        }
    }

    pub fn id(&self) -> i32 {
        self.id
    }

    pub fn address(&self) -> u8 {
        self.address as u8
    }

    pub fn increase_timeout(&mut self) {
        self.timeout += 50;
    }
    pub fn timeout(&self) -> u64 {
        self.timeout
    }

    pub fn is_active(&self) -> bool {
        self.is_active
    }

    pub fn total_steps(&self) -> usize {
        self.value_pool.total_steps()
    }

    pub fn retry_count(&self) -> i32 {
        self.retry_count
    }

    pub fn set_values(&mut self, values: Vec<ValueRead>){
        self.value_pool.init(values, self.read_by_group);
    }
    pub fn get_pool(&self) -> PoolValueRefIterator<'_> {
        self.value_pool.into_iter()
    }
    pub fn finished(&mut self) {
       self.next_read_at = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")  // упаде, то й похуй, це ж раст...
            .as_millis() as u64 + self.time_for_recall as u64;
    }
    pub fn when_next(&self) -> u64 {
        self.next_read_at
    }

}

