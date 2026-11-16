// read_master_struct.rs
use std::io::ErrorKind;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, oneshot};
use tokio::time::{sleep, timeout, Instant};
use tokio_modbus::client::{tcp, Context};
use tokio_modbus::{Error, ExceptionCode, ProtocolError, Slave};
use tokio_modbus::prelude::{Reader, SlaveContext};
use crate::messages::main_msg::MainMsg;
use crate::reader::reader_loop::modbus_device::modbus_device_unit::ModbusDeviceUnit;

use crate::db::schemas::{
    node::NodeRead,
    device::DeviceRead,
    value_unit::ValueRead
};
use crate::logger::printers;
use crate::messages::requests::device_request::{DeviceRequest, GetDeviceByNode};
use crate::messages::requests::request_struct::Request;
use crate::messages::requests::value_request::{GetByDeviceId, ValueRequest};
use crate::messages::events::{
event::Event,
node_event::{NodeEvent, NodeEventType},
device_event::{DeviceEvent, DeviceEventType}
};
use crate::reader::reader_loop::decoding_plugins::value_interface::RegType;

#[derive(Debug, Clone, Copy)]
enum ModbusReadError {
    Timeout,
    DeviceProtocolError,
    SocketFail,
    RemoveStep,
    HeaderMisMatch
}

enum HeaderMismatchAction {
    None,
    IncreaseCurrent,
    IncreasePrevious,
}

type ModbusReadResult =
Result<Vec<u16>, ModbusReadError>;

pub struct ReadMaster{
    id: i32,
    ip: Arc<String>,
    port: i32,
    devices: Vec<ModbusDeviceUnit>,
    ctx: Option<Context>,
    current_devise_index: Option<usize>,
    lastest_device_index: Option<usize>,
    to_controller: mpsc::Sender<MainMsg>,
    ctx_state: NodeEventType,
    last_connecting_time: Instant,
}
impl ReadMaster{
    pub fn new(config: &NodeRead, sender: mpsc::Sender<MainMsg>) -> ReadMaster{
        ReadMaster{
            id: config.id,
            ip: Arc::new(config.ip.clone()),
            port: config.port.unwrap_or(502),
            devices: Vec::new(),
            ctx: None,
            current_devise_index: None,
            lastest_device_index: None,
            to_controller: sender,
            ctx_state: NodeEventType::UnConnected,
            last_connecting_time: Instant::now()
        }
    }
    pub fn id(&self) -> i32{
        self.id
    }
    pub fn ip(&self) -> Arc<String>{
        self.ip.clone()
    }
    pub async fn update(&mut self, conf: Arc<NodeRead>) {
        if self.ip.as_str() != conf.ip || self.port != conf.port.unwrap_or(502) {
            self.ip = Arc::new(conf.ip.clone());
            self.port = conf.port.unwrap_or(502);
            let _ = self.create_context().await;
        }
        let _ = self.create_devices().await;
        let msg = format!("Ноду оновлено! IP: {}, PORT: {}", &self.ip, &self.port);
        printers::event(msg);
    }

    pub fn when_next(&mut self) -> u64 {
        if self.devices.is_empty() {
            return 60_000;
        }
        let deadline = if self.ctx.is_none() {
            self.last_connecting_time
        } else {
            self.when_next_device()
        };
        deadline.saturating_duration_since(Instant::now()).as_millis() as u64
    }

    pub async fn tick(&mut self) {
        if self.devices.is_empty() && !(self.create_devices().await) {
            return;
        }
        // пристрої є, шо там з контекстом?
        if self.ctx.is_none() && !(self.create_context().await){
            return;
        }

        self.read().await;
    }

    async fn read(&mut self) {

        let mut total_result = Vec::new();
        let mut header_mismatch = HeaderMismatchAction::None;
        let mut failed_steps = 0;
        let total_steps;
        let dev_id;

        {
            let device = if let Some(idx) = self.current_devise_index {
                &mut self.devices[idx]
            } else {
                return;
            };


            if !device.is_active() {
                return;
            }

            let ctx = match self.ctx.as_mut() {
                Some(ctx) => ctx,
                None => return,
            };
            let slave = Slave(device.address());
            ctx.set_slave(slave);

            dev_id = device.id();
            total_steps = device.total_steps(); // ?


            let timeout_duration = device.timeout();
            let retry_count = device.retry_count() as u64;

            'step_loop: for step in device.get_pool() {
                if step.is_broken() {
                    failed_steps += 1;
                    continue
                };
                let mut curr_try = 0;
                loop {
                    let read_result = match step.get_type() {
                        RegType::Input => {
                            read_process(|| ctx.read_input_registers(step.get_start(), step.get_length()),
                                         timeout_duration,
                                         dev_id,
                                         self.ip.clone(),
                            ).await
                        },
                        RegType::Holding => {
                            read_process(|| ctx.read_holding_registers(step.get_start(), step.get_length()),
                                         timeout_duration,
                                         dev_id,
                                         self.ip.clone(),
                            ).await
                        }
                        RegType::Coils => {
                            read_process(|| ctx.read_coils(step.get_start(), step.get_length()),
                                         timeout_duration,
                                         dev_id,
                                         self.ip.clone(),
                            ).await
                        },
                        RegType::Discrete => {
                            read_process(|| ctx.read_discrete_inputs(step.get_start(), step.get_length()),
                                         timeout_duration,
                                         dev_id,
                                         self.ip.clone(),
                            ).await
                        },
                    };

                    match read_result {
                        Ok(read_result) => {
                            for val in step.get_value(&read_result) {
                                total_result.push(val);
                            }
                            break;
                        },
                        Err(e) => {
                            match e {
                                ModbusReadError::Timeout | ModbusReadError::DeviceProtocolError => {
                                    curr_try += 1;
                                },
                                ModbusReadError::SocketFail => {
                                    self.disconnecting().await;
                                    return;
                                },
                                ModbusReadError::RemoveStep => {
                                    step.mark_broken();
                                    printers::err(format!("Заблоковані регістри пристрою id: {}, адреси з {} по {}  для ноди :{}",
                                                          dev_id,
                                                          step.get_start(),
                                                          (u32::from(step.get_start()) + u32::from(step.get_length())).saturating_sub(1),
                                                          self.ip.clone()));
                                    for val in step.fail() {
                                        total_result.push(val);
                                    }
                                    failed_steps += 1;
                                    break;
                                },
                                ModbusReadError::HeaderMisMatch => {
                                    if curr_try > 0 {
                                        header_mismatch  = HeaderMismatchAction::IncreaseCurrent;
                                    } else {
                                        header_mismatch  = HeaderMismatchAction::IncreasePrevious;
                                    }
                                    break 'step_loop;
                                }
                            }
                        }
                    }
                    if curr_try >= retry_count {
                        for val in step.fail() {
                            total_result.push(val);
                        }
                        failed_steps += 1;
                        break;
                    }
                };

            }
        }
        let prev = self.lastest_device_index;
        self.lastest_device_index = self.current_devise_index;

        match header_mismatch {
            HeaderMismatchAction::None => {},
            HeaderMismatchAction::IncreaseCurrent => {
                self.increase_timeout(self.current_devise_index).await;
                return;
            },
            HeaderMismatchAction::IncreasePrevious => {
                self.increase_timeout(prev).await;
                return;
            },
        }

        let idx = match self.current_devise_index {
            Some(idx) => idx,
            None => return, // провсяккий випадок
        };
        let device = &mut self.devices[idx];

        let read_report = match failed_steps {
            0 => DeviceEventType::Full,
            _ if failed_steps == total_steps => DeviceEventType::Failed,
            _ => DeviceEventType::SomePart,
        };

        let res = DeviceEvent{
            event: read_report,
            id: dev_id,
            measures: total_result,
        };

        let res = MainMsg::Event(
            Event::DeviceEvent(
                Arc::new(res),
            )
        );

        device.finished();
        let send_result = self.to_controller.send(res).await;
        if self.devices.len() > 1{
            sleep(Duration::from_millis(20)).await;
        }
        match send_result {
            Ok(_) => {
            },
            Err(e) => {
                printers::err(format!("Помилка відправки результатів опитування: {:?}", e));
            }
        }
    }

    async fn increase_timeout(&mut self, idx: Option<usize>) {
        if let Some(idx) = idx {
            let device = &mut self.devices[idx];
            device.increase_timeout();
            printers::warn(format!("Збільшено таймаут для пристрою id: {}, адреса: {}, нода: {}, таймаут: {}", device.id(), device.address(), self.ip.clone(), device.timeout()));
        }
        self.disconnecting().await; // чистимо буфер дісконектом...
        sleep(Duration::from_millis(5000)).await;
        if !self.create_context().await {
            self.disconnecting().await;
        }
    }

    fn when_next_device(&mut self) -> Instant {
        let next = self.devices.iter().enumerate()
            .filter(|(_, device)| device.is_active())
            .map(|(idx, device)| (idx, device.when_next()))
            .min_by_key(|&(_, deadline)| deadline);
        self.current_devise_index = next.map(|(idx, _)| idx);
        next.map(|(_, deadline)| deadline)
            .unwrap_or_else(|| Instant::now() + Duration::from_secs(1))
    }

    async fn create_context(&mut self) -> bool {
        self.ctx = None;  // про всяк випадок чистимо старий контекст
        self.start_connecting().await;
        let socket_addr = match format!("{}:{}", self.ip, self.port).parse() {
            Ok(addr) => addr,
            Err(e) => {
                printers::err(format!("Не вірна адреса сокету: {}", e));
                self.disconnecting().await;
                return false;
            }
        };
        let res = timeout(Duration::from_secs(5), tcp::connect(socket_addr)).await;

        match res {
            Ok(Ok(ctx)) => {
                let msg = format!("Створено контекст для ip: {}", &self.ip);
                printers::event(msg);
                self.ctx = Some(ctx);
                self.connecting_finished().await;
                true
            },
            Ok(Err(e)) => {
                let msg = format!("Помилка створення контексту для ip: {}, \nerr: {} \nПовторна спроба через 60 сек.", &self.ip, e);
                printers::err(msg);
                self.disconnecting().await;
                false
            }
            Err(_) => {
                printers::warn(format!("Таймаут підключення до ip: {}", self.ip));
                self.disconnecting().await;
                false
            }
        }
    }

    async fn create_devices(&mut self) -> bool { // true = devices was created
        self.devices.clear();
        self.current_devise_index = None;
        self.lastest_device_index = None;
        let devices = self.get_devises().await;
        if devices.is_empty() {
            return false;
        }
        let mut devices_units = Vec::with_capacity(devices.len());
        for device_config in devices {
            let new_dev_unit = self.get_device_from_db(device_config).await;
            if let Some(new_dev_unit) = new_dev_unit {
                devices_units.push(new_dev_unit);
            }
        }
        self.devices = devices_units;
        let _ = self.when_next_device();
        !self.devices.is_empty()
    }

    async fn get_device_from_db(&mut self, device_config: DeviceRead) -> Option<ModbusDeviceUnit> {
        let mut new_dev_unit = ModbusDeviceUnit::new(device_config);
        let values = self.get_values_from_db(new_dev_unit.id()).await;
        if let Some(values) = values {
            new_dev_unit.set_values(values);
            Some(new_dev_unit)
        } else {
            None
        }
    }

    async fn get_values_from_db(&mut self, id: i32) -> Option<Vec<ValueRead>> {
        let (tx, rx) = oneshot::channel::<Result<Vec<ValueRead>, ()>>();
        let send_res = self.to_controller.send(Self::create_value_request(id, tx)).await;
        match send_res {
            Ok(_) => {},
            Err(e) => {
                printers::err(format!("Помилка відправки запиту до БД на отримання значень\nid:{}\nErr: {}", id, e));
                return None;
            }
        }
        let res = rx.await;
        let values = match res {
            Ok(msg) => {
                match msg {
                    Ok(values_res) => {
                        values_res
                    },
                    Err(_) => {
                        printers::err(format!("Помилка  отримання значень\nid:{}\nПомилка бази даних", id));
                        return None;
                    }
                }
            },
            Err(e) => {
                printers::err(format!("Помилка каналу на отримання значень\nid:{}\nErr: {}", id, e));
                return None;
            }
        };
        Some(values)
    }

    async fn get_devises(&mut self) -> Vec<DeviceRead> {
        let mut counter = 0;
        let devices ;

        loop {
            let (tx, rx) = oneshot::channel();

            let dev_request = GetDeviceByNode {
                node_id: self.id,
                request_channel: tx,
            };
            let dev_request = DeviceRequest::GetByNode(dev_request);
            let dev_request = Request::GetDevice(dev_request);
            let dev_request = MainMsg::Request(dev_request);

            let send_res = self.to_controller.send(dev_request).await;
            match send_res {
                Ok(_) => {},
                Err(e) => {
                    let msg = format!("Помилка відправки повідомлення до контролера при створенні пристроїв для {}\nErr: {}", &self.ip, e);
                    printers::err(msg);
                }
            }

            let res = rx.await;

            match res {
                Ok(msg) => {
                    match msg {
                        Ok(devices_res) => {
                            devices = devices_res;
                            break;
                        },
                        Err(_) => {
                            let msg = format!("Помилка отримання даних від бази даних для {}", &self.ip);
                            printers::err(msg);
                        }
                    }
                },
                Err(e) => {
                    let msg = format!("Помилка каналу отримування відповіді для {} від бази: {}", &self.ip, e);
                    printers::err(msg);
                }
            }
            sleep(std::time::Duration::from_millis(100)).await;
            counter += 1;
            if counter >= 5 {
                let msg = format!("Помилка отримання пристроїв для {}, кількість спроб вичерпано", &self.ip);
                printers::warn(msg);
                return Vec::new();
            }
        }
        devices
    }

    fn create_value_request(dev_id: i32, chan: oneshot::Sender<Result<Vec<ValueRead>, ()>>) -> MainMsg {
        let val_request = GetByDeviceId {
            device_id: dev_id,
            request_channel: chan,
        };
        let val_request = ValueRequest::GetByDeviceId(val_request);
        let val_request = Request::GetValue(val_request);
        MainMsg::Request(val_request)
    }
    async fn start_connecting(&mut self) {
        self.send_connecting_msg(NodeEventType::Connecting).await;
        printers::event(format!("Підключення до ip: {}", &self.ip));
    }
    async fn connecting_finished(&mut self) {
        self.send_connecting_msg(NodeEventType::Connected).await;
        printers::event(format!("Підключено до ip: {}", &self.ip));
    }

    async fn disconnecting(&mut self) {
        self.last_connecting_time = Instant::now() + Duration::from_secs(60);
        self.send_connecting_msg(NodeEventType::UnConnected).await;
        printers::err(format!("Закрито з'єднання ip: {}", &self.ip));
        self.ctx = None
    }
    async fn send_connecting_msg(&mut self, msg: NodeEventType) {
        self.ctx_state = msg;

        let send_msg = MainMsg::Event(
            Event::NodeEvent(
                Arc::new(
                    NodeEvent{
                        event: msg,
                        id: self.id,
                        ip: self.ip.clone(),
                    }
                )
            )
        );
        let send_res = self.to_controller.send(send_msg).await;
        match send_res {
            Ok(_) => {},
            Err(e) => {
                let msg = format!("Помилка відправки повідомлення до контролера про початок створення {}\nErr: {}", &self.ip, e);
                printers::err(msg);
            }
        }
    }
}

async fn read_process<F, Fut, Word>(
    f: F,
    timeout_duration: u64,
    device_id: i32,
    node_ip: Arc<String>,
) -> ModbusReadResult
where
    F: FnOnce() -> Fut,
// Трейт-баунд під будь-яку таску читання з tokio_modbus
    Fut: Future<Output = Result<Result<Vec<Word>, ExceptionCode>, Error>>,
// Гарантуємо, що Word (u16 або bool) можна скастити в u16
    Word: Into<u16> + Copy,
{
    let timeout_duration = Duration::from_millis(timeout_duration);
    let res = timeout(timeout_duration, f()).await;
    match res {
        Ok(Ok(data)) => {
            match data {
                Ok(raw_values) => {
                    let cleaned_data = raw_values.iter().map(|&val| val.into()).collect::<Vec<u16>>();
                    Ok(cleaned_data)
                },
                Err(e) => {
                    match e {
                        ExceptionCode::IllegalFunction => {
                            printers::err(format!("Пристрій не підтримує функцію запиту\n    id: {} нода {}", device_id, node_ip));
                            Err(ModbusReadError::RemoveStep)
                        },
                        ExceptionCode::IllegalDataAddress => {
                            printers::err(format!("Запитані регістри не існують\n    id: {} нода {}", device_id, node_ip));
                            Err(ModbusReadError::RemoveStep)
                        },
                        ExceptionCode::IllegalDataValue | ExceptionCode::MemoryParityError => {
                            printers::err(format!("Біда з даними/паритетом (CRC?)\n    id: {} нода {}", device_id, node_ip));
                            Err(ModbusReadError::Timeout) // Повертаємо як таймаут, нехай пробує ще
                        },
                        ExceptionCode::ServerDeviceBusy | ExceptionCode::Acknowledge => {
                            // Пристрій зайнятий. Спимо 100 мс прямо тут і повертаємо Timeout для ретраю
                            Err(ModbusReadError::Timeout)
                        },
                        ExceptionCode::ServerDeviceFailure | ExceptionCode::GatewayPathUnavailable | ExceptionCode::GatewayTargetDevice => {
                            // Щось із залізом або шлюзом. Шлемо на перепідключення сокета
                            Err(ModbusReadError::SocketFail)
                        },
                        _ => Err(ModbusReadError::Timeout),
                    }
                }
            }
        }
        Ok(Err(e)) => {
            match e {
                Error::Transport(io_err) => {
                    match io_err.kind() {
                        ErrorKind::ConnectionRefused  |
                        ErrorKind::ConnectionReset    |
                        ErrorKind::HostUnreachable    |
                        ErrorKind::NetworkUnreachable |
                        ErrorKind::ConnectionAborted  |
                        ErrorKind::NotConnected       |
                        ErrorKind::BrokenPipe         |
                        ErrorKind::TimedOut => {
                            // Відправляємо сигнал на create_context
                            Err(ModbusReadError::SocketFail)
                        },

                        ErrorKind::NetworkDown => {
                            printers::err(String::from("Упала мережа системи,"));
                            Err(ModbusReadError::SocketFail)
                        },

                        other => {
                            printers::warn(format!("Дивна помилка IO при читанні ModbusTCP:\n {:?}", other));
                            Err(ModbusReadError::Timeout)
                        }
                    }
                },
                Error::Protocol(protocol_error) => {
                    match protocol_error {
                        ProtocolError::FunctionCodeMismatch { .. } => {
                            printers::warn(format!("Код відповіді не відповідає запиту\n id: {}, для ноди {}", device_id, node_ip));
                            Err(ModbusReadError::DeviceProtocolError)
                        },
                        ProtocolError::HeaderMismatch { message, .. } => {
                            printers::warn(format!("Заголовок відповіді не відповідає запиту: {}\n id: {}, для ноди {}", message, device_id, node_ip));
                            Err(ModbusReadError::HeaderMisMatch)
                        },
                    }
                }
            }
        }
        Err(_) => {
            Err(ModbusReadError::Timeout)
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    fn master() -> ReadMaster {
        let (tx, _rx) = mpsc::channel(10);
        ReadMaster::new(&NodeRead {
            id: 1,
            ip: "127.0.0.1".into(),
            port: None,
            description: None,
        }, tx)
    }

    fn device(id: i32, active: bool) -> ModbusDeviceUnit {
        ModbusDeviceUnit::new(DeviceRead {
            id,
            device_name: None,
            address: id,
            parent_node_id: 1,
            time_for_recall: 15_000,
            timeout: 200,
            retry_count: 3,
            is_active: active,
            read_by_group: true,
            description: None,
            deleted: Some(false),
            deleted_at: None,
        })
    }

    #[test]
    fn inactive_first_device_does_not_block_active_devices() {
        let mut master = master();
        master.devices = vec![device(6, false), device(1, true), device(2, true)];
        let deadline = master.when_next_device();
        assert_eq!(master.current_devise_index, Some(1));
        assert_eq!(deadline, master.devices[1].when_next());
    }

    #[test]
    fn empty_and_inactive_lists_have_a_real_idle_deadline() {
        let mut master = master();
        for devices in [vec![], vec![device(6, false)]] {
            master.devices = devices;
            master.current_devise_index = Some(99);
            let before = Instant::now();
            let deadline = master.when_next_device();
            assert_eq!(master.current_devise_index, None);
            assert!(deadline >= before + Duration::from_secs(1));
            assert!(deadline <= Instant::now() + Duration::from_secs(1));
        }
    }

    #[test]
    fn earliest_active_device_is_selected() {
        let mut master = master();
        let mut later = device(1, true);
        later.finished();
        master.devices = vec![later, device(2, true), device(6, false)];
        master.when_next_device();
        assert_eq!(master.current_devise_index, Some(1));
    }

    #[tokio::test]
    async fn failed_value_lookups_leave_an_empty_list_without_panicking() {
        let mut master = master();
        master.current_devise_index = Some(99);
        master.lastest_device_index = Some(99);
        let (tx, mut rx) = mpsc::channel(10);
        master.to_controller = tx;
        let responder = tokio::spawn(async move {
            let Some(MainMsg::Request(Request::GetDevice(
                DeviceRequest::GetByNode(request)
            ))) = rx.recv().await else { panic!("expected device lookup") };
            // Build a database configuration from the same values as the table.
            request.request_channel.send(Ok(vec![DeviceRead {
                id: 1, device_name: None, address: 4, parent_node_id: 1,
                time_for_recall: 20_000, timeout: 200, retry_count: 3,
                is_active: true, read_by_group: true, description: None,
                deleted: Some(false), deleted_at: None,
            }])).unwrap();
            let Some(MainMsg::Request(Request::GetValue(
                ValueRequest::GetByDeviceId(request)
            ))) = rx.recv().await else { panic!("expected value lookup") };
            request.request_channel.send(Err(())).unwrap();
        });
        assert!(!timeout(Duration::from_secs(3), master.create_devices())
            .await.expect("device initialization must complete"));
        assert!(master.devices.is_empty());
        assert_eq!(master.current_devise_index, None);
        assert_eq!(master.lastest_device_index, None);
        responder.await.unwrap();
    }

    #[tokio::test]
    async fn device_lookup_stops_after_five_errors() {
        let mut master = master();
        let (tx, mut rx) = mpsc::channel(10);
        master.to_controller = tx;
        let responder = tokio::spawn(async move {
            let mut attempts = 0;
            while let Some(MainMsg::Request(Request::GetDevice(
                DeviceRequest::GetByNode(request)
            ))) = rx.recv().await {
                attempts += 1;
                request.request_channel.send(Err(())).unwrap();
            }
            attempts
        });
        let devices = timeout(Duration::from_secs(3), master.get_devises())
            .await.expect("device lookup must stop retrying");
        assert!(devices.is_empty());
        drop(master);
        assert_eq!(responder.await.unwrap(), 5);
    }
}
