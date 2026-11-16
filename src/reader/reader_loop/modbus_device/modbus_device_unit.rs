use std::time::Duration;
use tokio::time::Instant;

use crate::db::schemas::device::DeviceRead;
use crate::db::schemas::value_unit::ValueRead;
use crate::reader::reader_loop::modbus_device::pool_iterator::{PollValueIterator, PoolValueRefIterator};

const MAX_ADAPTIVE_TIMEOUT_MS: u64 = 60_000;


pub struct ModbusDeviceUnit {
    id: i32,
    _name: String,
    address: i32,
    time_for_recall: i32,
    timeout: u64,
    retry_count: i32,
    is_active: bool,
    read_by_group: bool,
    next_read_at: Instant,
    value_pool: PollValueIterator,
}

impl ModbusDeviceUnit {
    pub fn new(setting: DeviceRead) -> ModbusDeviceUnit {

        let time = Instant::now();

        ModbusDeviceUnit {
            id: setting.id,
            _name: setting.device_name.unwrap_or_default(),
            address: setting.address,
            timeout: setting.timeout.max(1) as u64,
            time_for_recall: setting.time_for_recall.max(1),
            retry_count: setting.retry_count.max(1),
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
        // Bound automatic adaptation without reducing a larger configured timeout.
        if self.timeout < MAX_ADAPTIVE_TIMEOUT_MS {
            self.timeout = self.timeout.saturating_add(50).min(MAX_ADAPTIVE_TIMEOUT_MS);
        }
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
        self.next_read_at = Instant::now()
            + Duration::from_millis(self.time_for_recall as u64);
    }
    pub fn when_next(&self) -> Instant {
        self.next_read_at
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    fn device(recall: i32, timeout: i32, retries: i32) -> ModbusDeviceUnit {
        ModbusDeviceUnit::new(DeviceRead {
            id: 1,
            device_name: None,
            address: 4,
            parent_node_id: 1,
            time_for_recall: recall,
            timeout,
            retry_count: retries,
            is_active: true,
            read_by_group: true,
            description: None,
            deleted: Some(false),
            deleted_at: None,
        })
    }

    #[test]
    fn supplied_intervals_produce_expected_deadlines() {
        for recall in [15_000, 20_000] {
            let mut device = device(recall, 200, 3);
            let before = Instant::now();
            device.finished();
            let interval = Duration::from_millis(recall as u64);
            assert!(device.when_next() >= before + interval);
            assert!(device.when_next() <= Instant::now() + interval);
        }
    }

    #[test]
    fn invalid_database_timings_do_not_become_unsigned_maxima() {
        let mut device = device(-1, -1, -1);
        assert_eq!(device.timeout(), 1);
        assert_eq!(device.retry_count(), 1);
        device.finished();
        assert!(device.when_next() <= Instant::now() + Duration::from_millis(1));
    }

    #[test]
    fn automatic_timeout_increases_are_bounded() {
        let mut device = device(15_000, 200, 3);
        for _ in 0..2_000 {
            device.increase_timeout();
        }
        assert_eq!(device.timeout(), 60_000);
    }
}
