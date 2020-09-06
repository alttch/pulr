use std::cell::RefCell;
use std::collections::HashMap;
use std::time::{Duration, Instant};

#[derive(Debug)]
struct ValSpeedInfo {
    value: f64,
    last: Instant,
}

thread_local!(static SPEED_INFO: RefCell<HashMap<u64, ValSpeedInfo>>
    = RefCell::new(HashMap::new()));

fn calculate_growth_speed(
    value: f64,
    uid: u64,
    maxval: f64,
    interval: f64,
    t: Instant,
) -> Option<f64> {
    let mut speed: Option<f64> = None;
    SPEED_INFO.with(|speed_info_cell| {
        let mut sinfo = speed_info_cell.borrow_mut();
        let prev = sinfo.get(&uid);
        speed = match prev {
            Some(prv) => {
                let t_delta: Duration = t - prv.last;
                if t_delta < Duration::from_millis((interval * 1000.0) as u64) {
                    None
                } else {
                    let v_delta: f64;
                    if value >= prv.value {
                        v_delta = value - prv.value;
                    } else {
                        v_delta = maxval - prv.value + value;
                    }
                    Some(v_delta / (t_delta.as_millis() / 1000) as f64)
                }
            }
            None => Some(0.0),
        };
        if speed.is_some() {
            sinfo.insert(
                uid,
                ValSpeedInfo {
                    value: value,
                    last: t,
                },
            );
        }
    });
    return speed;
}

pub trait Transform {
    fn multiply(&self, multiplier: f64) -> f64;
    fn divide(&self, divisor: f64) -> f64;
    fn round_to(&self, digits: f64) -> f64;
    fn to_num(&self) -> f64;
    fn to_bool(&self) -> bool;
    fn calc_speed(&self, uid: u64, interval: f64, t: Instant) -> Option<f64>;
}

macro_rules! impl_Transform_N {
    ($t:ty, $max:path) => {
        impl Transform for $t {
            fn multiply(&self, multiplier: f64) -> f64 {
                return *self as f64 * multiplier;
            }
            fn divide(&self, divisor: f64) -> f64 {
                return *self as f64 / divisor;
            }
            fn round_to(&self, digits: f64) -> f64 {
                return round_to(*self as f64, digits);
            }
            fn to_num(&self) -> f64 {
                return *self as f64;
            }
            fn to_bool(&self) -> bool {
                return *self != 0 as $t;
            }
            fn calc_speed(&self, uid: u64, interval: f64, t: Instant) -> Option<f64> {
                return calculate_growth_speed(*self as f64, uid, $max as f64, interval, t);
            }
        }
    };
}

fn round_to(value: f64, digits: f64) -> f64 {
    if digits >= 20.0 {
        panic!("max round: 19 digits ({})", digits);
    }
    let m: f64 = (10 as u64).pow(digits as u32) as f64;
    return (value * m).round() / m;
}

fn str_to_f64(s: &String) -> f64 {
    match s.parse::<f64>() {
        Ok(v) => v,
        Err(_) => panic!("Unable to parse number: {}", s),
    }
}

impl Transform for String {
    fn multiply(&self, multiplier: f64) -> f64 {
        return str_to_f64(self) * multiplier;
    }
    fn divide(&self, divisor: f64) -> f64 {
        return str_to_f64(self) / divisor;
    }
    fn round_to(&self, digits: f64) -> f64 {
        return round_to(str_to_f64(self), digits);
    }
    fn to_num(&self) -> f64 {
        return str_to_f64(self);
    }
    fn to_bool(&self) -> bool {
        return str_to_f64(self) != 0.0;
    }
    fn calc_speed(&self, _uid: u64, _interval: f64, _t: Instant) -> Option<f64> {
        unimplemented!("unable to calculate speed for string");
    }
}

impl Transform for bool {
    fn multiply(&self, _multiplier: f64) -> f64 {
        unimplemented!();
    }
    fn divide(&self, _divisor: f64) -> f64 {
        unimplemented!();
    }
    fn round_to(&self, _digits: f64) -> f64 {
        unimplemented!();
    }
    fn to_num(&self) -> f64 {
        match self {
            true => 1.0,
            false => 0.0,
        }
    }
    fn to_bool(&self) -> bool {
        return *self;
    }
    fn calc_speed(&self, _uid: u64, _interval: f64, _t: Instant) -> Option<f64> {
        unimplemented!("unable to calculate speed for boolean");
    }
}

impl_Transform_N!(i8, std::i8::MAX);
impl_Transform_N!(u8, std::u8::MAX);
impl_Transform_N!(i16, std::i16::MAX);
impl_Transform_N!(u16, std::u16::MAX);
impl_Transform_N!(i32, std::i32::MAX);
impl_Transform_N!(u32, std::u32::MAX);
impl_Transform_N!(i64, std::i64::MAX);
impl_Transform_N!(u64, std::u64::MAX);
impl_Transform_N!(f32, std::f32::MAX);
impl_Transform_N!(f64, std::f64::MAX);

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum TransformFunction {
    Multiply,
    Divide,
    Round,
    CalcSpeed,
    //To_Num,
    //To_Bool,
}
