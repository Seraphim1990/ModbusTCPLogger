use std::collections::HashMap;
use sqlx::{MySql, Pool};
use crate::db::hasher::{
  db_measure_unit::DbMeasureUnit,
  hash_unit::ValueHasher
};
use tokio::sync::mpsc;
use crate::messages::main_msg::MainMsg;
use crate::messages::{
    requests::{
        request_struct::Request,
        measure_request::MeasureRequest,
    },
    events::{
        event::Event,
        device_event::DeviceEvent
    }
};
use crate::logger::printers;
use crate::messages::requests::measure_request::{HashedValue, MeasureResponse};

const MAX_PONT_IN_WEEK: usize = 131072; // орієнтовний вирівняний розмір на 2 тижні, можна змінити 65536(~50к записів має бути в тижні)

pub async fn measure_master(pool: Pool<MySql>, mut from_controller: mpsc::Receiver<MainMsg>) {

    let mut hashed_map : HashMap<i32, ValueHasher> = HashMap::new();
    let mut db_unit = DbMeasureUnit::new(pool);
    while let Some(msg) = from_controller.recv().await {
        match msg { 
            MainMsg::Command(_) => {
                printers::err("Контролері вимірів не має отримувати MainMsg::Command".to_string())
            },
            MainMsg::Request(req) => {
                if let Request::GetMeasure(measure_req) = req {
                    let results = read_measure(&measure_req, &mut hashed_map, &mut db_unit).await;
                    if measure_req.response_sender.send(results).is_err() {
                        printers::err("Помилка відправлення відповіді MainMsg::Request".to_string());
                    }
                } else {
                    printers::err("measure_master має приймати запити тільки GetMeasure".to_string());
                }
            },
            MainMsg::Event(ev) => {
                if let Event::DeviceEvent(dev_event) = ev {
                    write_measure(dev_event.as_ref(), &mut hashed_map, &mut db_unit).await;
                } else {
                    printers::err("measure_master має приймати події тільки DeviceEvent".to_string())
                }
            }
        }
    }
}


async fn read_measure(req: &MeasureRequest, hash_map: &mut HashMap<i32, ValueHasher>, db_unit: &mut DbMeasureUnit) -> Result<Vec<MeasureResponse>, String>  {
    let mut results : Vec<MeasureResponse> = Vec::with_capacity(req.values_id.len());
    for val_id in &req.values_id {

        if let Some(hasher) = hash_map.get_mut(val_id) &&
            let Some(res) = hasher.get_hashed(req.from, req.to) {

            results.push(MeasureResponse { id: *val_id, values: res, from: req.from, to: req.to });
            continue;
        }
        let res = db_unit.get_measures(*val_id, req.from, req.to).await?;
        results.push(MeasureResponse {id: *val_id, values: res, from: req.from, to: req.to});
    }
    Ok(results)
}

async fn write_measure(ev: &DeviceEvent, hash_map: &mut HashMap<i32, ValueHasher>, db_unit: &mut DbMeasureUnit) {
    for value in ev.measures.iter() {
        if !value.is_logging {continue}
        match hash_map.entry(value.value_id as i32) {
            std::collections::hash_map::Entry::Occupied(mut entry) => {
                let hasher = entry.get_mut();
                let (now, before) = hasher.add(value.measure_value, value.measure_time);
                for val in [before, now].into_iter().flatten() { // .flatten() ігнорує None і розпаковує Some
                    if db_unit.save_value(value.value_id as i32, val).await.is_err() {
                        printers::err("Помилка зберігання значення в базу".to_string())
                    }
                }
            },
            std::collections::hash_map::Entry::Vacant(entry) => {
                let new_hasher = ValueHasher::new(MAX_PONT_IN_WEEK, value.measure_value, value.measure_time);
                entry.insert(new_hasher);
                let val_to_save = HashedValue{val: value.measure_value, timestamp: value.measure_time};

                if db_unit.save_value(value.value_id as i32, val_to_save).await.is_err() {
                    printers::err("Помилка зберігання значення в базу".to_string())
                }
            }
        }
    }
}



