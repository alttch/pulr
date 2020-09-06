pub trait Transform {
    fn multiply(&self, multiplier: f64) -> f64;
    fn divide(&self, divisor: f64) -> f64;
    fn round_to(&self, digits: f64) -> f64;
    fn to_num(&self) -> f64;
    fn to_bool(&self) -> bool;
}

macro_rules! impl_Transform_N {
    (for $($t:ty),+) => {
        $(impl Transform for $t {
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
        })*
    }
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
}

impl_Transform_N!(for i8, u8, i16, u16, i32, u32, i64, u64, f32, f64);

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum TransformFunction {
    Multiply,
    Divide,
    Round,
    //To_Num,
    //To_Bool,
}
