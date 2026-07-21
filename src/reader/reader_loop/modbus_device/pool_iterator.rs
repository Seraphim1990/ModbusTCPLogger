use crate::db::schemas::value_unit::ValueRead;
use crate::reader::reader_loop::decoding_plugins::plugin_loader::{get_plugin, RegisterDecodingPlugin};
use crate::reader::reader_loop::decoding_plugins::value_interface::RegType;
use crate::reader::reader_loop::modbus_device::read_steps::ReadStep;
use crate::logger::printers;


struct InitPlugins{
    values: Vec<RegisterDecodingPlugin>,
    registers: Vec<i32>,
    reg_type: RegType,
}

pub struct PoolValueRefIterator<'a>{
    pool: &'a PollValueIterator,
    curr_type: RegType,
    curr_index: usize,
}

impl<'a> Iterator for PoolValueRefIterator<'a>{
    type Item = &'a ReadStep;
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let curr_list = match self.curr_type {
                RegType::Coils => &self.pool.coils,
                RegType::Discrete => &self.pool.discrete,
                RegType::Holding => &self.pool.holding,
                RegType::Input => &self.pool.input,
            };
            if self.curr_index < curr_list.len(){
                let step = &curr_list[self.curr_index];
                self.curr_index += 1;
                return Some(step);
            } else {
                match self.curr_type {
                    RegType::Coils => { self.curr_index = 0; self.curr_type = RegType::Discrete; },
                    RegType::Discrete => { self.curr_index = 0; self.curr_type = RegType::Holding; },
                    RegType::Holding => { self.curr_index = 0; self.curr_type = RegType::Input; },
                    RegType::Input => { return None },
                }
            }
        }
    }
}

pub struct PollValueIterator {
    coils: Vec<ReadStep>,
    discrete: Vec<ReadStep>,
    holding: Vec<ReadStep>,
    input: Vec<ReadStep>,
}

impl<'a> IntoIterator for &'a PollValueIterator {
    type Item = &'a ReadStep;
    type IntoIter = PoolValueRefIterator<'a>;
    fn into_iter(self) -> Self::IntoIter {
        PoolValueRefIterator {
            pool: self,
            curr_index: 0,
            curr_type: RegType::Coils,
        }
    }
}

impl PollValueIterator {
    pub fn new()  -> PollValueIterator {
        PollValueIterator {
            coils: Vec::new(),
            discrete: Vec::new(),
            holding: Vec::new(),
            input: Vec::new(),
        }
    }

    pub fn total_steps(&self) -> usize {
        self.coils.len()+self.discrete.len()+self.holding.len()+self.input.len()
    }

    pub fn contains_value_id(&self, value_id: i32) -> bool {
        self.coils.iter()
            .chain(self.discrete.iter())
            .chain(self.holding.iter())
            .chain(self.input.iter())
            .flat_map(|step| step.get_values_id())
        .any(|x| x == value_id)
    }

    pub fn init(&mut self, values: Vec<ValueRead>, read_by_group: bool) {
        self.coils.clear();
        self.discrete.clear();
        self.holding.clear();
        self.input.clear();

        let (coils, discrete, holding, input) = PollValueIterator::create_units(values, read_by_group);
        self.coils = coils;
        self.discrete = discrete;
        self.holding = holding;
        self.input = input;
    }

    fn create_units(values: Vec<ValueRead>, by_group: bool) -> (Vec<ReadStep>, Vec<ReadStep>, Vec<ReadStep>, Vec<ReadStep>) {
        if by_group {
            Self::group_create(values)
        } else {
            Self::single_create(values)
        }
    }

    fn single_create(values: Vec<ValueRead>)-> (Vec<ReadStep>, Vec<ReadStep>, Vec<ReadStep>, Vec<ReadStep>) {

        let (coils, discrete, holding, input) = Self::init_plugins(values);

        (Self::single_sort(coils),
         Self::single_sort(discrete),
         Self::single_sort(holding),
         Self::single_sort(input),)
    }

    fn single_sort(values: InitPlugins) -> Vec<ReadStep> {
        let registers = values.registers;
        let reg_type = values.reg_type;
        let mut values = values.values;

        let mut res = Vec::new();

        for reg in registers {
            let mut step = ReadStep::new(reg, 1, reg_type);

            let addr = vec!(reg);
            let mut inexes = Vec::new();
            for (i, val) in values.iter_mut().enumerate() {
                if val.find_your_registers(&addr) {
                    inexes.push(i);
                }
            }

            inexes.sort();
            inexes.reverse();
            for i in inexes {
                step.add_unit(values.remove(i));
            }
            res.push(step);
        }
        res
    }


    fn group_create(values: Vec<ValueRead>) -> (Vec<ReadStep>, Vec<ReadStep>, Vec<ReadStep>, Vec<ReadStep>) {
        let (coils, discrete, holding, input) = Self::init_plugins(values);

        (Self::group_sort(coils),
        Self::group_sort(discrete),
        Self::group_sort(holding),
        Self::group_sort(input))
    }

    fn group_sort(values: InitPlugins) -> Vec<ReadStep> {
        let registers = Self::reg_sort(values.registers);
        let reg_type = values.reg_type;
        let mut values = values.values;

        let mut res = Vec::new();

        for reg in &registers {
            let mut step = ReadStep::new(*reg.first().unwrap(), reg.len() as i32, reg_type);
            let mut inexes = Vec::new();
            for (i, val) in values.iter_mut().enumerate() {
                if val.find_your_registers(reg) {
                    inexes.push(i);
                }
            }
            inexes.sort();
            inexes.reverse();
            for i in inexes {
                step.add_unit(values.remove(i));
            }
            res.push(step);
        }
        res
    }

    fn reg_sort(mut registers: Vec<i32>) -> Vec<Vec<i32>> {
        registers.sort();
        let mut res = Vec::new();
        let mut group: Vec<i32> = Vec::new();
        for reg in registers {
            if group.is_empty() || reg == *group.last().unwrap() + 1 {
                group.push(reg);
            } else {
                res.push(std::mem::take(&mut group));
                group.push(reg);
            }
        }
        if !group.is_empty() {
            res.push(group);
        }
        res
    }

    fn init_plugins(values: Vec<ValueRead>)-> (InitPlugins, InitPlugins, InitPlugins, InitPlugins) {
        let mut units_coil = InitPlugins {
            values: Vec::new(),
            registers: Vec::new(),
            reg_type: RegType::Coils,
        };
        let mut units_discrete = InitPlugins{
            values: Vec::new(),
            registers: Vec::new(),
            reg_type: RegType::Discrete,
        };
        let mut units_holding = InitPlugins{
            values: Vec::new(),
            registers: Vec::new(),
            reg_type: RegType::Holding,
        };
        let mut units_input = InitPlugins {
            values: Vec::new(),
            registers: Vec::new(),
            reg_type: RegType::Input,
        };

        for value in values {
            let settings = value.settings.to_string();
            let mut plugin = match get_plugin(value.decoding_type) {
                Ok(plugin) => plugin,
                Err(e) => {
                    let msg = format!("Помилка завантаження плагіну: {e}, \n значення : {:?} ", value);
                    printers::err(msg);
                    continue;
                }
            };

            let addrs = plugin.init(settings, value.id, value.is_logging);
            match plugin.get_type() {
                RegType::Coils => {
                    units_coil.values.push(plugin);
                    for i in addrs {
                        units_coil.registers.push(i);
                    }
                }
                RegType::Discrete => {
                    units_discrete.values.push(plugin);
                    for i in addrs {
                        units_discrete.registers.push(i);
                    }
                }
                RegType::Holding => {
                    units_holding.values.push(plugin);
                    for i in addrs {
                        units_holding.registers.push(i);
                    }
                }
                RegType::Input => {
                    units_input.values.push(plugin);
                    for i in addrs {
                        units_input.registers.push(i);
                    }
                }

            }
        }

        (units_coil, units_discrete, units_holding, units_input)
    }
}