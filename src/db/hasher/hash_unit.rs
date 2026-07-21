use crate::messages::requests::measure_request::HashedValue;

const STABLE_DELTA: i64 = 300;
const FAILURE_DELTA: i64 = 120;

enum HashState{
    Normal,
    InFail,
    FailReported
}
pub struct ValueHasher {
    hashed_values: Vec<HashedValue>,
    size: usize,
    capacity: usize,
    current: usize,
    stable_delta: i64,
    failure_delta: i64,
    in_failure: bool,
    failure_reported: bool,

    last_emitted_ts: i64,

    head: usize,
    tail: usize,
    ring_finished: bool,
    is_resolving_duplicate: bool,
    state: HashState,
}

impl ValueHasher {
    pub fn new(capacity: usize, val: f64, timestamp: i64) -> ValueHasher {
        let mut hashed_values = vec![HashedValue::default(); capacity];
        hashed_values[0] = HashedValue { val, timestamp };
        ValueHasher {
            hashed_values,
            size: 1,
            current: 0,
            capacity,
            stable_delta: STABLE_DELTA,
            failure_delta: FAILURE_DELTA,
            in_failure: false,
            failure_reported: false,
            last_emitted_ts: timestamp,

            head: 0,
            tail: 0,
            ring_finished: false,
            is_resolving_duplicate: false,
            state: HashState::Normal,
        }
    }

    pub fn add(&mut self, val: f64, timestamp: i64) -> (Option<HashedValue>, Option<HashedValue>) {

        // не прочитано
        // цей блок має працювати без реворку
        if val == f64::MIN {
            return self.add_fail(val, timestamp);
        }
        // прочитано, нормальний режим
        if !self.failure_reported && !self.in_failure {  // Normal
            return self.add_normal(val, timestamp);
        }
        // прочитано, це був шум (failure не встигла зареєструватись)
        if self.in_failure && !self.failure_reported { // InFail
            return self.add_after_noice(val, timestamp);
        }
        // прочитано, failure вже була зареєстрована (Err уже пішов) — виходимо з режиму
        if self.failure_reported { // FailReported
           return self.clean_fail_report(val, timestamp);
        }
        Self::void()
    }

    /// Компенсує рух head в go_ahead: рахує позиції та фіксує момент,
    /// коли кільце вперше заповнюється повністю.
    fn advance(&mut self) {
        if !self.ring_finished {
            self.current += 1;
            if self.current == self.capacity {
                self.ring_finished = true;
            }
        }
    }
    fn go_ahead(&mut self) {
        self.head = (self.head + 1) % self.capacity;
        if self.ring_finished {
            self.tail = (self.capacity + self.head + 1) % self.capacity;
        }
    }
    fn roll_back(&mut self) {
        // формально, фільтується в get_hashed
        // хвіст не повинен рухатись назад, бо head записав туди дані, які вважаються шумом!
        if self.ring_finished{
            self.head = if self.head == 0 { self.capacity - 1 } else { self.head - 1 };
        } else {
            if self.head != 0 {self.head -= 1} else {self.head = 0}
        }
    }
    fn add_normal(&mut self, val: f64, timestamp: i64) -> (Option<HashedValue>, Option<HashedValue>) {
        if self.hashed_values[self.head].val == val { // нічого не помінялось
            if (timestamp - self.last_emitted_ts) > self.stable_delta {
                self.advance();
                self.go_ahead();
                self.hashed_values[self.head] = HashedValue { val, timestamp };
                self.last_emitted_ts = timestamp;
                self.is_resolving_duplicate = false;
                return self.current_val()
            } else {
                self.is_resolving_duplicate = true;
                self.hashed_values[self.head].timestamp = timestamp;
            }
        } else { // таки помінялось
            self.advance();
            self.go_ahead();
            self.hashed_values[self.head] = HashedValue { val, timestamp };
            self.last_emitted_ts = timestamp;
            return if self.is_resolving_duplicate {
                self.is_resolving_duplicate = false;
                self.current_and_previous_val()
            } else { self.current_val() }
        }
        Self::void()
    }
    fn add_after_noice(&mut self, val: f64, timestamp: i64) -> (Option<HashedValue>, Option<HashedValue>) {
        self.in_failure = false;
        if !self.ring_finished {
            self.current -= 1; // компенсуємо advance() з MIN-гілки
        }
        self.roll_back(); // відкочуємо голову на слот до входу в failure

        self.add_normal(val, timestamp)
    }

    fn clean_fail_report(&mut self, val: f64, timestamp: i64) -> (Option<HashedValue>, Option<HashedValue>) {
        self.in_failure = false;
        self.failure_reported = false;
        self.is_resolving_duplicate = false;
        self.advance();
        self.go_ahead();
        self.hashed_values[self.head] = HashedValue { val, timestamp };
        self.last_emitted_ts = timestamp;
        self.current_val()
    }

    fn add_fail(&mut self, val: f64, timestamp: i64) -> (Option<HashedValue>, Option<HashedValue>) {
        if self.failure_reported {
            return Self::void()
        }
        if !self.in_failure {
            self.advance();
            self.go_ahead();
            self.hashed_values[self.head] = HashedValue { val, timestamp };
            self.in_failure = true; // фільтр
        }
        if (timestamp - self.hashed_values[self.head].timestamp) > self.failure_delta
                && !self.failure_reported {
            self.hashed_values[self.head].timestamp = timestamp;
            self.failure_reported = true;
            return if self.is_resolving_duplicate {
                self.is_resolving_duplicate = false;
                self.current_and_previous_val()
            } else { self.current_val() }
        }
        Self::void()
    }

    fn current_val(&self) -> (Option<HashedValue>, Option<HashedValue>) {
        (Some(self.hashed_values[self.head].clone()), None)
    }
    fn current_and_previous_val(&self) -> (Option<HashedValue>, Option<HashedValue>) {
        if self.head == 0 && !self.ring_finished{
            return Self::void()
        }
        let prev = if self.head == 0 && self.ring_finished  {
            self.capacity - 1
        } else {
            self.head - 1
        };
        (Some(self.hashed_values[self.head].clone()), Some(self.hashed_values[prev].clone()))
    }
    fn void() -> (Option<HashedValue>, Option<HashedValue>) {
        (None, None)
    }

    // код старого add
    /*
if val == f64::MIN {
    if !self.in_failure {
        self.advance_and_write(val, timestamp);
        self.in_failure = true;
        // last_emitted_ts НЕ оновлюємо — бо це початок failure
        return Ok(None);
    }

    if !self.failure_reported {
        if timestamp - self.hashed_values[self.current].timestamp >= self.failure_delta {
            self.hashed_values[self.current].timestamp = timestamp;
            self.failure_reported = true;

            self.last_emitted_ts = timestamp;   // <-- важливо!
            return Err(self.hashed_values[self.current].clone());
        }
        return Ok(None);
    }
    return Ok(None);
}
// ====================== val != f64::MIN ======================

if self.failure_reported {
    self.in_failure = false;
    self.failure_reported = false;
    self.advance_and_write(val, timestamp);
    self.last_emitted_ts = timestamp;
    return Ok(Some(self.hashed_values[self.current].clone()));
}

if self.in_failure {
    self.in_failure = false;
    let prev_idx = self.current.checked_sub(1)
        .unwrap_or(self.capacity - 1);

    let prev_val = self.hashed_values[prev_idx].val;
    let time_since_emitted = timestamp - self.last_emitted_ts;

    if val != prev_val || time_since_emitted >= self.stable_delta {
        self.hashed_values[self.current] = HashedValue { val, timestamp };
        self.last_emitted_ts = timestamp;
        return Ok(Some(self.hashed_values[self.current].clone()));
    }

    // failure була шумом — відкочуємо каретку
    self.current = prev_idx;
    self.hashed_values[self.current].timestamp = timestamp;

    // Виправлення: компенсуємо advance_and_write
    if self.size > 0 {
        self.size -= 1;
    }

    return Ok(None);
}
// ==================== Нормальний режим ====================
let prev_val = self.hashed_values[self.current].val;
let time_since_emitted = timestamp - self.last_emitted_ts;

if val != prev_val || time_since_emitted >= self.stable_delta {
    self.advance_and_write(val, timestamp);
    self.last_emitted_ts = timestamp;
    return Ok(Some(self.hashed_values[self.current].clone()));
}

// те саме значення, ще рано видавати
self.hashed_values[self.current].timestamp = timestamp;
Ok(None)

 */

    fn on_loop(&mut self, val: f64, timestamp: i64) -> Result<Option<HashedValue>, HashedValue> {
        unimplemented!()
    }

    fn pre_loop(&mut self, val: f64, timestamp: i64) -> Result<Option<HashedValue>, HashedValue> {
        unimplemented!()
    }

    pub fn get_hashed(&self, from: i64, to: i64) -> Option<Vec<HashedValue>> {
        if from >= to
            || (self.head == self.tail && !self.ring_finished)
            || from < self.hashed_values[self.tail].timestamp
        {
            return None;
        }

        let oldest_idx = self.tail;
        let len = (self.head + self.capacity - oldest_idx) % self.capacity + 1;

        let start = Self::lower_bound_ring(&self.hashed_values, len, oldest_idx, from);
        let end = Self::lower_bound_ring(&self.hashed_values, len, oldest_idx, to);

        if start == end {
            return None;
        }

        let mut result = Vec::with_capacity(end - start);
        for i in start..end {
            let idx = (oldest_idx + i) % self.capacity;
            let hv = &self.hashed_values[idx];
            if hv.timestamp != 0 { // захист про всяк випадок
                result.push(hv.clone());
            }
        }

        Some(result)
    }


    fn lower_bound_ring(
        arr: &[HashedValue],
        size: usize,
        oldest_idx: usize,
        target: i64,
    ) -> usize {
        let len = arr.len();
        let mut left = 0usize;
        let mut right = size;

        while left < right {
            let mid = left + (right - left) / 2;
            let idx = (oldest_idx + mid) % len;

            if arr[idx].timestamp < target {
                left = mid + 1;
            } else {
                right = mid;
            }
        }
        left
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    fn hv(val: f64, ts: i64) -> HashedValue {
        HashedValue { val, timestamp: ts }
    }

    #[test]
    fn test_initial_state() {
        let mut hasher = ValueHasher::new(5, 10.0, 1000);  // timestamp завжди росте!
        // перший add після new — повинен повернути значення
        assert_eq!(hasher.add(10.0, 1001), (None, None)); // значення не змінилось!!!
        assert_eq!(hasher.add(11.0, 1002), (Some(hv(11.0, 1002)), Some(hv(10.0, 1001)))); // значення змінилось!
    }

    #[test]
    fn test_stable_duplicate() {
        let mut hasher = ValueHasher::new(5, 10.0, 1000);

        assert_eq!(hasher.add(10.0, 1100), (None, None));     // ще рано
        assert_eq!(hasher.add(10.0, 1400), (Some(hv(10.0, 1400)), None)); // після STABLE_DELTA
    }

    #[test]
    fn test_value_change() {
        let mut hasher = ValueHasher::new(5, 10.0, 1000);

        let (curr, prev) = hasher.add(20.0, 1100);
        assert_eq!(curr, Some(hv(20.0, 1100)));
        assert_eq!(prev, None);
    }

    #[test]
    fn test_duplicate_resolution() {
        let mut hasher = ValueHasher::new(5, 10.0, 1000);

        let (curr, last) = hasher.add(20.0, 1100);
        assert_eq!(curr, Some(hv(20.0, 1100)));
        assert_eq!(last, None);

        let (curr, last) = hasher.add(20.0, 1200); // тихий дубль, is_resolving_duplicate = true
        assert_eq!(curr, None);
        assert_eq!(last, None);

        let (curr, last) = hasher.add(20.0, 1300); // ще один тихий дубль, слот оновлено на 1300
        assert_eq!(curr, None);
        assert_eq!(last, None);

        let (curr, prev) = hasher.add(30.0, 1600); // змінилось — забираємо і нову, і незалоговану стару точку
        assert_eq!(curr, Some(hv(30.0, 1600)));
        assert_eq!(prev, Some(hv(20.0, 1300)));
    }

    #[test]
    fn test_failure_detection() {
        let mut hasher = ValueHasher::new(5, 10.0, 1000);

        assert_eq!(hasher.add(f64::MIN, 1050), (None, None));
        assert_eq!(hasher.add(f64::MIN, 1100), (None, None));
        let (curr, prev) = hasher.add(f64::MIN, 1300);
        assert_eq!(curr, Some(hv(f64::MIN, 1300)));
        assert_eq!(prev, None);
    }

    #[test]
    fn test_failure_noise_recovery() {
        let mut hasher = ValueHasher::new(5, 10.0, 1000);

        hasher.add(f64::MIN, 1050);                    // failure
        let (curr, prev) = hasher.add(15.0, 1150);     // шум → відновлення

        assert_eq!(curr, Some(hv(15.0, 1150)));
        assert_eq!(prev, None);   // або Some, залежно від логіки
    }

    #[test]
    fn test_get_hashed_basic() {
        let mut hasher = ValueHasher::new(10, 10.0, 1000);
        hasher.add(20.0, 1100);
        hasher.add(30.0, 1200);

        let result = hasher.get_hashed(1000, 1300);
        assert!(result.is_some());
    }
    #[test]
    fn test_timeout_checkpoint_not_reemitted_as_prev() {
        let mut hasher = ValueHasher::new(5, 10.0, 1000);

        // checkpoint після stable_delta — залоговано одразу
        let (curr, _) = hasher.add(10.0, 1400);
        assert_eq!(curr, Some(hv(10.0, 1400)));

        // значення міняється одразу після — checkpoint вище НЕ повинен
        // повторно піти як prev, він уже в базі
        let (curr, prev) = hasher.add(20.0, 1450);
        assert_eq!(curr, Some(hv(20.0, 1450)));
        assert_eq!(prev, None); // а зараз, підозрюю, поверне Some(hv(10.0, 1400)) — дубль
    }
    #[test]
    fn test_get_hashed_empty_state() {
        let hasher = ValueHasher::new(5, 10.0, 1000);
        // head == tail і кільце ще не завершене — даних по суті ще немає
        assert_eq!(hasher.get_hashed(900, 2000), None);
    }

    #[test]
    fn test_get_hashed_pre_loop_partial() {
        let mut hasher = ValueHasher::new(5, 10.0, 1000); // pre-loop, capacity 5
        hasher.add(20.0, 1100);
        hasher.add(30.0, 1200);
        // заповнено лише 3 з 5 слотів, tail лишається на 0

        let result = hasher.get_hashed(1000, 1300).unwrap();
        assert_eq!(result, vec![hv(10.0, 1000), hv(20.0, 1100), hv(30.0, 1200)]);
    }

    #[test]
    fn test_get_hashed_ring_wrap_momentum() {
        let mut hasher = ValueHasher::new(3, 10.0, 1000); // capacity 3, щоб швидко замкнути кільце
        hasher.add(20.0, 1100);
        hasher.add(30.0, 1200);
        hasher.add(40.0, 1300); // тут ring_finished стає true, head перестрибує на 0

        // перевіряємо, що tail коректно "ожив" і хронологія не зламалась,
        // попри те що head фізично повернувся на індекс 0
        let result = hasher.get_hashed(1100, 1400).unwrap();
        assert_eq!(result, vec![hv(20.0, 1100), hv(30.0, 1200), hv(40.0, 1300)]);
    }

    #[test]
    fn test_get_hashed_ring_wrap_standard_step() {
        let mut hasher = ValueHasher::new(3, 10.0, 1000);
        hasher.add(20.0, 1100);
        hasher.add(30.0, 1200);
        hasher.add(40.0, 1300); // замкнули кільце
        hasher.add(50.0, 1400); // ще один звичайний крок on-loop

        // tail = head + 1 має триматись стабільно після кожного кроку
        let result = hasher.get_hashed(1200, 1500).unwrap();
        assert_eq!(result, vec![hv(30.0, 1200), hv(40.0, 1300), hv(50.0, 1400)]);
    }

    #[test]
    fn test_get_hashed_skips_stale_slot_after_noise_gap() {
        let mut hasher = ValueHasher::new(3, 10.0, 1000);
        hasher.add(20.0, 1100);
        hasher.add(30.0, 1200);
        hasher.add(40.0, 1300); // ring_finished, head=0, tail=1

        hasher.add(f64::MIN, 1350);   // вхід у failure, записується в новий слот
        hasher.add(40.0, 1360);       // шум: значення таке саме, як до failure -> roll_back, gap = 2

        // слот з f64::MIN лишається "за межами" [tail, head] і не повинен потрапити в результат
        let result = hasher.get_hashed(1200, 1400).unwrap();
        assert_eq!(result, vec![hv(30.0, 1200), hv(40.0, 1360)]);
        assert!(!result.iter().any(|v| v.val == f64::MIN));
    }

    #[test]
    fn test_get_hashed_out_of_range_queries() {
        let mut hasher = ValueHasher::new(5, 10.0, 1000);
        hasher.add(20.0, 1100);
        hasher.add(30.0, 1200);

        assert_eq!(hasher.get_hashed(500, 900), None);   // запит раніше за найстаріші дані
        assert_eq!(hasher.get_hashed(1200, 1100), None); // from >= to
        let partial = hasher.get_hashed(1150, 5000).unwrap();
        assert_eq!(partial, vec![hv(30.0, 1200)]);        // частковий збіг з хвоста діапазону
    }
}