const ERROR_SYSTEM_TIME: &str = "invalid system time";

use chrono::{Local, TimeZone};
use serde::{ser::SerializeMap, Deserialize, Deserializer, Serialize, Serializer};
use std::collections::hash_map::DefaultHasher;
use std::error::Error;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::time::{SystemTime, UNIX_EPOCH};
use transform;

pub fn calculate_hash<T: Hash>(t: &T) -> u64 {
    let mut s = DefaultHasher::new();
    t.hash(&mut s);
    return s.finish();
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum TimeFormat {
    Omit,
    Raw,
    Rfc3339,
}

// time
pub fn get_default_time_format() -> TimeFormat {
    return TimeFormat::Omit;
}

#[derive(Debug)]
pub struct TimeFormatError;

impl Error for TimeFormatError {}

impl fmt::Display for TimeFormatError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Invalid time format")
    }
}

pub fn get_time_format(time_format: &String) -> Result<TimeFormat, TimeFormatError> {
    return match (time_format).to_lowercase().as_str() {
        "" => Ok(TimeFormat::Omit),
        "rfc3339" => Ok(TimeFormat::Rfc3339),
        "raw" | "timestamp" => Ok(TimeFormat::Raw),
        _ => Err(TimeFormatError {}),
    };
}

// output

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum OutputType {
    Stdout,
    StdoutCsv,
    StdoutNdJson,
    StdoutEvaDatapuller,
}

pub fn get_default_output() -> OutputType {
    return OutputType::Stdout;
}

#[derive(Debug)]
pub struct OutputTypeError;

impl Error for OutputTypeError {}

impl fmt::Display for OutputTypeError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Invalid output type")
    }
}

pub fn get_output_type(output_type: &String) -> Result<OutputType, OutputTypeError> {
    return match output_type.to_lowercase().as_str() {
        "stdout" | "text" | "plain" | "-" => Ok(OutputType::Stdout),
        "csv" => Ok(OutputType::StdoutCsv),
        "ndjson" | "json" => Ok(OutputType::StdoutNdJson),
        "eva/datapuller" | "eva" => Ok(OutputType::StdoutEvaDatapuller),
        _ => Err(OutputTypeError {}),
    };
}

// event

fn de_transform_task<'de, D>(
    deserializer: D,
) -> serde::export::Result<transform::TransformFunction, D::Error>
where
    D: Deserializer<'de>,
{
    let func = String::deserialize(deserializer).unwrap();
    return Ok(match func.as_str() {
        "multiply" => transform::TransformFunction::Multiply,
        "divide" => transform::TransformFunction::Divide,
        "round" => transform::TransformFunction::Round,
        _ => unimplemented!("function {}", func),
    });
}

pub fn empty_transform_task() -> EventTransformList {
    return vec![];
}

#[derive(serde::Deserialize, Debug)]
pub struct EventTransformTask {
    #[serde(deserialize_with = "de_transform_task")]
    func: transform::TransformFunction,
    args: Vec<f64>,
}

pub type EventTransformList = Vec<EventTransformTask>;

#[derive(Clone, Copy, Debug)]
pub struct EventTime {
    time: SystemTime,
    time_format: TimeFormat,
}

impl EventTime {
    pub fn new(time_format: TimeFormat) -> Self {
        return Self {
            time: SystemTime::now(),
            time_format: time_format,
        };
    }

    pub fn as_secs(&self) -> f64 {
        return self
            .time
            .duration_since(UNIX_EPOCH)
            .expect(ERROR_SYSTEM_TIME)
            .as_millis() as f64
            / 1000.0;
    }
}

impl ToString for EventTime {
    fn to_string(&self) -> String {
        return match self.time_format {
            TimeFormat::Raw => self.as_secs().to_string(),
            TimeFormat::Rfc3339 => {
                let dur = self
                    .time
                    .duration_since(UNIX_EPOCH)
                    .expect(ERROR_SYSTEM_TIME);
                let sec = dur.as_secs() as i64;
                let nsec = dur.subsec_nanos();
                return Local.timestamp(sec, nsec).to_rfc3339();
            }
            _ => "".to_owned(),
        };
    }
}

pub struct Event<'a, T: ToString> {
    pub id: &'a String,
    pub id_hash: u64,
    pub value: T,
    pub t: &'a EventTime,
    pub transform_list: &'a EventTransformList,
}

impl<'a, T: serde::Serialize + std::fmt::Display> Serialize for Event<'a, T> {
    fn serialize<S>(&self, serializer: S) -> serde::export::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut map = serializer.serialize_map(Some(3)).unwrap();
        map.serialize_entry("id", &self.id).unwrap();
        map.serialize_entry("value", &self.value).unwrap();
        match self.t.time_format {
            TimeFormat::Raw => map.serialize_entry("time", &self.t.as_secs()).unwrap(),
            TimeFormat::Rfc3339 => map.serialize_entry("time", &self.t.to_string()).unwrap(),
            _ => {}
        }
        //map.serialize_entry("time", &self.t.to_string());
        return map.end();
    }
}

impl<'a, T: ToString + transform::Transform> Event<'a, T> {
    pub fn new(
        id: &'a String,
        value: T,
        transform: &'a EventTransformList,
        t: &'a EventTime,
    ) -> Self {
        return Event {
            id: id,
            id_hash: calculate_hash(id),
            value: value,
            t: t,
            transform_list: transform,
        };
    }
    pub fn transform(
        &self,
        transform_function: transform::TransformFunction,
        args: &Vec<f64>,
    ) -> Option<Event<f64>> {
        let value = match transform_function {
            transform::TransformFunction::Multiply => self.value.multiply(*args.get(0).unwrap()),
            transform::TransformFunction::Divide => self.value.divide(*args.get(0).unwrap()),
            transform::TransformFunction::Round => self.value.round_to(*args.get(0).unwrap()),
        };
        return Some(Event {
            id: &self.id,
            id_hash: self.id_hash,
            value: value,
            t: self.t,
            transform_list: &self.transform_list,
        });
    }
    pub fn transform_at(&self, ti: usize) -> Option<Event<f64>> {
        let tr = self.transform_list.get(ti).unwrap();
        return self.transform(tr.func, &tr.args);
    }
}

// work data types

#[derive(PartialEq, Eq, Debug)]
pub enum GenDataType {
    Bit,
    Int8,
    Int16,
    Int32,
    Uint8,
    Uint16,
    Uint32,
    Uint64,
    Int64,
    Real32,
    Real64,
}

impl std::fmt::Display for GenDataType {
    fn fmt(&self, fmt: &mut std::fmt::Formatter) -> std::result::Result<(), std::fmt::Error> {
        return write!(fmt, "{:?}", self);
    }
}

pub trait GenDataTypeParse {
    fn parse_data_type(&self) -> GenDataType;
}

impl GenDataTypeParse for String {
    fn parse_data_type(&self) -> GenDataType {
        return match self.to_lowercase().as_str() {
            "bit" => GenDataType::Bit,
            "uint8" | "byte" => GenDataType::Uint8,
            "int8" | "sint8" => GenDataType::Uint8,
            "uint16" | "word" => GenDataType::Uint16,
            "int16" | "sint16" => GenDataType::Int16,
            "uint32" | "dword" => GenDataType::Uint32,
            "int32" | "sint32" => GenDataType::Int32,
            "uint64" | "qword" => GenDataType::Uint64,
            "int64" | "sint64" => GenDataType::Uint64,
            "real32" | "real" | "float32" | "float" => GenDataType::Real32,
            "real64" | "float64" => GenDataType::Real64,
            _ => unimplemented!("Unsupported data type: {}", self),
        };
    }
}

pub struct DataOffset {
    pub offset: usize,
    pub bit: Option<u8>,
}

impl Default for DataOffset {
    fn default() -> Self {
        DataOffset {
            offset: 0,
            bit: None,
        }
    }
}

pub trait ParseData {
    fn parse_data_offset(&self, addr: u32) -> DataOffset;
    fn safe_parse_u32(&self) -> u32;
}

impl ParseData for String {
    fn parse_data_offset(&self, addr: u32) -> DataOffset {
        let mut i = self.split("/");
        let mut o = i.next().unwrap().to_string();
        let bit: Option<u8> = match i.next() {
            Some(v) => Some(v.parse().unwrap()),
            None => None,
        };
        let mut offset: u32;
        if o.chars().next().unwrap() == '=' {
            o.remove(0);
            offset = o.safe_parse_u32();
            offset = offset - addr;
        } else {
            offset = o.safe_parse_u32();
        }
        return DataOffset {
            offset: offset as usize,
            bit: bit,
        };
    }
    fn safe_parse_u32(&self) -> u32 {
        let mut a: u32 = 0;
        for val in self.split("+") {
            a = a + val
                .parse::<u32>()
                .expect(&format!("unable to parse number from {}", self));
        }
        return a;
    }
}
