use core::time::Duration;
use sqlx::{MySql, Pool};
use sqlx::__rt::timeout;
use crate::messages::requests::measure_request::{HashedValue};
use crate::logger::printers;
use sqlx::QueryBuilder;

struct ForFlush{
    id: i32,
    value: HashedValue,
}

pub struct DbMeasureUnit {
    pool: Pool<MySql>,
    buff : Vec<ForFlush>,
}

impl DbMeasureUnit {
    pub fn new(pool: Pool<MySql>) -> Self {
        DbMeasureUnit {
            buff : Vec::with_capacity(60),
            pool
        }
    }
    pub async fn get_measures(&self, val_id: i32, from: i64, to: i64) -> Result<Vec<HashedValue>, String> {

        match timeout(Duration::from_secs(20),
                       sqlx::query_as::<_, HashedValue>(
            "
            SELECT
            measure_value as val,
            measure_time as timestamp
            FROM measures
            WHERE value_id = ?
            AND measure_time >= ?
            AND measure_time <= ?
            ORDER BY measure_time
            "
            )
            .bind(val_id)
            .bind(from)
            .bind(to)
            .fetch_all(&self.pool)
        ).await
        {
            Ok(Ok(res)) => {Ok(res)}
            Ok(Err(e)) => {
                let msg = format!("Помилка читання вимірів із бази даних: {:?}", e);
                printers::err(msg.clone());
                Err(msg)
            }
            Err(_) => {
                let msg = "Таймаут читання з бази даних".to_string();
                printers::err(msg.clone());
                Err(msg)
            }
        }
    }
    pub async fn save_value(&mut self, val_id: i32, val: HashedValue) -> Result<(), ()> {
        let saved_value = ForFlush {id: val_id, value: val};
        self.buff.push(saved_value);
        if self.buff.len() > 20 {
            return timeout(Duration::from_secs(10),
                           self.flush()) // flush -> Result<(), ()>
                .await
                .map_err(|_| {
                    printers::err("Таймаут збереження в базу даних".to_string());
                })?;
        };
        Ok(())
    }
    async fn flush(&mut self) -> Result<(), ()> {
        if self.buff.is_empty() {
            return Ok(());
        }
        let mut tx = self.pool.begin()
            .await
            .map_err(|e| {
                printers::err(format!("Помилка відкриття транзакції для збереження буферу вимірів: {}", e));
            })?;

        let mut builder = QueryBuilder::new(
            "INSERT INTO measures (value_id, measure_value, measure_time) "
        );

        builder.push_values(&self.buff, |mut b, m| {
            b.push_bind(m.id)
                .push_bind(m.value.val)
                .push_bind(m.value.timestamp);
        });

        builder
            .build()
            .execute(&mut *tx)
            .await
            .map_err(|e| {
                printers::err(format!("Помилка завершення транзакції для збереження буферу вимірів: {}", e));
            })?;

        tx.commit().await.map_err(|e| {
            printers::err(format!("Помилка завершення транзакції для збереження буферу вимірів: {}", e));
        })?;
        self.buff.clear();
        Ok(())
    }
}