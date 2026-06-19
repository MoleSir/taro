use std::collections::HashMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use crate::{NativeFunction, ObjectHandle, ShrString};
use crate::vm::{RuntimeResult, RuntimeErrorKind, VirtualMachine};

impl VirtualMachine {
    /// Create the `time` std module.
    ///
    /// # Exports
    ///
    /// | function      | description                                  |
    /// |---------------|----------------------------------------------|
    /// | `time()`      | current Unix timestamp (seconds as float)    |
    /// | `sleep(secs)` | pause execution for `secs` seconds           |
    /// | `now()`       | current UTC time as a structured object      |
    ///
    /// The object returned by `now()` has these fields:
    ///   year, month (1-12), day (1-31), hour (0-23), min (0-59),
    ///   sec (0-59, fractional), wday (0=Sun..6=Sat), yday (1-366),
    ///   timestamp (Unix seconds as float)
    ///
    /// All fields are in UTC.
    pub(crate) fn create_time_module(&mut self) -> RuntimeResult<ObjectHandle> {
        let time_fn   = self.obj_heap.alloc_native_fn("time",   NativeFunction::a0(time));
        let sleep_fn  = self.obj_heap.alloc_native_fn("sleep",  NativeFunction::a1(sleep));
        let now_fn    = self.obj_heap.alloc_native_fn("now",    NativeFunction::a0(now));

        let mut exports: HashMap<ShrString, ObjectHandle> = HashMap::new();
        exports.insert(ShrString::new_str("time"),  time_fn);
        exports.insert(ShrString::new_str("sleep"), sleep_fn);
        exports.insert(ShrString::new_str("now"),   now_fn);

        let module = self.obj_heap.alloc_fields_instance(self.obj_heap.module_class, exports);
        Ok(module)
    }
}

// =====================================================================
//  Function implementations
// =====================================================================

/// `time.time()` — return the current Unix timestamp in seconds (f64).
fn time(vm: &mut VirtualMachine) -> RuntimeResult<ObjectHandle> {
    let dur = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    Ok(vm.obj_heap.alloc_float_instance(dur.as_secs_f64()))
}

/// `time.sleep(secs)` — pause execution for `secs` seconds (may be fractional).
fn sleep(vm: &mut VirtualMachine, secs: ObjectHandle) -> RuntimeResult<ObjectHandle> {
    let s = if let Ok(v) = vm.get_float_instance(secs) {
        *v
    } else if let Ok(v) = vm.get_integer_instance(secs) {
        *v as f64
    } else {
        return Err(RuntimeErrorKind::UnexpectedType("number", vm.value_type_name(secs)));
    };
    if s < 0.0 {
        return Err(RuntimeErrorKind::TimeError("sleep: negative duration".into()));
    }
    std::thread::sleep(Duration::from_secs_f64(s));
    Ok(ObjectHandle::NIL)
}

/// `time.now()` — return the current UTC time as a structured object.
///
/// The returned object has these fields:
///   year, month (1-12), day (1-31), hour (0-23), min (0-59),
///   sec (0-59, fractional), wday (0=Sun..6=Sat), yday (1-366),
///   timestamp (Unix seconds as float)
fn now(vm: &mut VirtualMachine) -> RuntimeResult<ObjectHandle> {
    let now_sys = SystemTime::now();
    let dur = now_sys.duration_since(UNIX_EPOCH).unwrap_or_default();
    let ts = dur.as_secs_f64();

    // Convert Unix timestamp to UTC calendar fields.
    let (year, month, day, hour, min, sec, wday, yday) = civil_from_seconds(dur.as_secs() as i64);

    let mut exports: HashMap<ShrString, ObjectHandle> = HashMap::new();
    exports.insert(ShrString::new_str("year"),      vm.obj_heap.alloc_integer_instance(year));
    exports.insert(ShrString::new_str("month"),     vm.obj_heap.alloc_integer_instance(month));
    exports.insert(ShrString::new_str("day"),       vm.obj_heap.alloc_integer_instance(day));
    exports.insert(ShrString::new_str("hour"),      vm.obj_heap.alloc_integer_instance(hour));
    exports.insert(ShrString::new_str("min"),       vm.obj_heap.alloc_integer_instance(min));
    // Include fractional seconds.
    let sec_frac = sec as f64 + dur.subsec_nanos() as f64 / 1_000_000_000.0;
    exports.insert(ShrString::new_str("sec"),       vm.obj_heap.alloc_float_instance(sec_frac));
    exports.insert(ShrString::new_str("wday"),      vm.obj_heap.alloc_integer_instance(wday));
    exports.insert(ShrString::new_str("yday"),      vm.obj_heap.alloc_integer_instance(yday));
    exports.insert(ShrString::new_str("timestamp"), vm.obj_heap.alloc_float_instance(ts));

    let class = vm.obj_heap.alloc_class("DataTime");
    let obj = vm.obj_heap.alloc_fields_instance(class, exports);
    Ok(obj)
}

// =====================================================================
//  UTC civil time conversion (Howard Hinnant's algorithm)
// =====================================================================

/// Convert a Unix timestamp (seconds since 1970-01-01 00:00:00 UTC) to
/// UTC calendar date/time components.
///
/// Returns `(year, month(1-12), day(1-31), hour(0-23), min(0-59), sec(0-59),
/// wday(0=Sun..6=Sat), yday(1-366))`.
fn civil_from_seconds(ts: i64) -> (i64, i64, i64, i64, i64, i64, i64, i64) {
    // Split into days and seconds-of-day.
    let z = ts / 86400;
    let mut day_secs = ts % 86400;
    if day_secs < 0 {
        day_secs += 86400;
    }

    let hour = day_secs / 3600;
    day_secs %= 3600;
    let min = day_secs / 60;
    let sec = day_secs % 60;

    // Day-of-week: 1970-01-01 was a Thursday (4).
    // (z + 4) mod 7, handling negative remainders.
    let wday = ((z + 4) % 7 + 7) % 7;

    // Convert days-since-epoch to civil date using Howard Hinnant's algorithm.
    // Shift epoch from 1970-01-01 to 0000-03-01.
    let z = z + 719468;
    let era = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe = z - era * 146097;                          // day of era [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // year of era [0, 399]
    let mut y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);   // day of "March year" [0, 365]
    let mp = (5 * doy + 2) / 153;                         // month-phase [0, 11]
    let d = doy - (153 * mp + 2) / 5 + 1;                // day of month [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 };       // month [1, 12]
    if m <= 2 { y += 1 }

    // Compute calendar day-of-year from (y, m, d).
    let yday = calendar_yday(y, m, d);

    (y, m, d, hour, min, sec, wday, yday)
}

/// Return the 1-based day-of-year for a given (year, month, day).
fn calendar_yday(y: i64, m: i64, d: i64) -> i64 {
    let leap = is_leap_year(y);
    let days_before: &[i64] = if leap {
        &[0, 0, 31, 60, 91, 121, 152, 182, 213, 244, 274, 305, 335]
    } else {
        &[0, 0, 31, 59, 90, 120, 151, 181, 212, 243, 273, 304, 334]
    };
    days_before[m as usize] + d
}

fn is_leap_year(y: i64) -> bool {
    y % 4 == 0 && (y % 100 != 0 || y % 400 == 0)
}

// =====================================================================
//  Tests
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_epoch_utc() {
        // 1970-01-01 00:00:00 UTC = timestamp 0
        let (y, m, d, h, min, s, wday, yday) = civil_from_seconds(0);
        assert_eq!(y, 1970);
        assert_eq!(m, 1);
        assert_eq!(d, 1);
        assert_eq!(h, 0);
        assert_eq!(min, 0);
        assert_eq!(s, 0);
        assert_eq!(wday, 4); // Thursday
        assert_eq!(yday, 1);
    }

    #[test]
    fn test_known_timestamp() {
        // 2024-01-01 00:00:00 UTC = 1704067200
        let ts = 1704067200;
        let (y, m, d, h, min, s, wday, yday) = civil_from_seconds(ts);
        assert_eq!(y, 2024);
        assert_eq!(m, 1);
        assert_eq!(d, 1);
        assert_eq!(h, 0);
        assert_eq!(min, 0);
        assert_eq!(s, 0);
        assert_eq!(wday, 1); // Monday
        assert_eq!(yday, 1);
    }

    #[test]
    fn test_mid_2024() {
        // 2024-07-15 12:30:45 UTC
        // 2024-07-15 is day 197 of 2024 (leap year)
        let ts = 1721046645;
        let (y, m, d, h, min, s, wday, yday) = civil_from_seconds(ts);
        assert_eq!(y, 2024);
        assert_eq!(m, 7);
        assert_eq!(d, 15);
        assert_eq!(h, 12);
        assert_eq!(min, 30);
        assert_eq!(s, 45);
        assert_eq!(wday, 1); // Monday
        assert_eq!(yday, 197);
    }
}
