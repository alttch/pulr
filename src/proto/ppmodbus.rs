use serde::{Deserialize, Deserializer};

use modbus::{Client, Coil};
use std::sync::mpsc;

use ieee754::Ieee754;

use std::thread;
use std::time::Duration;

use pl::tools::GetBit;
use pl::IntervalLoop;

use datatypes::{Event, EventTime, GenDataType, GenDataTypeParse, ParseData};
use pl::datatypes;

const DEFAULT_MODBUS_PORT: u16 = 502;

const ERROR_OOB: &str = "data out of bounds";

#[derive(Debug)]
enum ModbusRegisterType {
    Holding,
    Input,
    Coil,
    Discrete,
}

#[derive(Deserialize)]
struct ModbusConfig {
    proto: ModbusProto,
    pull: Vec<ModbusPull>,
}

define_de_source!(DEFAULT_MODBUS_PORT);

#[derive(Deserialize)]
struct ModbusProto {
    name: String,
    #[serde(deserialize_with = "de_source")]
    source: HostPort,
    unit: u8,
}

#[derive(Deserialize)]
struct ModbusPull {
    reg: String,
    count: u16,
    process: Vec<ModbusProcess>,
}

fn get_default_type() -> String {
    return "word".to_owned();
}

#[derive(Deserialize)]
struct ModbusProcess {
    offset: String,
    #[serde(alias = "type", default = "get_default_type")]
    r#type: String,
    #[serde(alias = "set-id")]
    set_id: String,
    #[serde(default = "datatypes::empty_transform_task")]
    transform: datatypes::EventTransformList,
}

// need coils as integers
trait CoilToNumber {
    fn to_number(&self) -> u16;
}

impl CoilToNumber for modbus::Coil {
    fn to_number(&self) -> u16 {
        return match self {
            Coil::On => 1,
            Coil::Off => 0,
        };
    }
}

// need to read coil data as Vec<u16>
trait ModbusDataAsU16 {
    fn read_coils_as_u16(&mut self, addr: u16, count: u16) -> Result<Vec<u16>, modbus::Error>;
    fn read_discrete_inputs_as_u16(
        &mut self,
        addr: u16,
        count: u16,
    ) -> Result<Vec<u16>, modbus::Error>;
}

impl ModbusDataAsU16 for modbus::tcp::Transport {
    fn read_coils_as_u16(&mut self, addr: u16, count: u16) -> Result<Vec<u16>, modbus::Error> {
        return match self.read_coils(addr, count) {
            Ok(v) => {
                let mut r = Vec::new();
                for c in v {
                    r.push(c.to_number());
                }
                Ok(r)
            }
            Err(e) => Err(e),
        };
    }
    fn read_discrete_inputs_as_u16(
        &mut self,
        addr: u16,
        count: u16,
    ) -> Result<Vec<u16>, modbus::Error> {
        return match self.read_coils(addr, count) {
            Ok(v) => {
                let mut r = Vec::new();
                for c in v {
                    r.push(c.to_number());
                }
                Ok(r)
            }
            Err(e) => Err(e),
        };
    }
}

// work structus

// TODO: move some fields to de_
struct ModbusDataProcessInfo {
    offset: datatypes::DataOffset,
    tp: datatypes::GenDataType,
    set_id: String,
    transform: datatypes::EventTransformList,
}

// TODO: move some fields to de_
#[derive(Debug)]
struct ModbusPullData {
    tp: ModbusRegisterType,
    addr: u16,
    count: u16,
}

define_task_result!(Vec<u16>);

pub fn run(
    inloop: bool,
    verbose: bool,
    cfg: String,
    timeout: Duration,
    interval: Duration,
    time_format: datatypes::TimeFormat,
    out: pl::Output,
    beacon: &mut pl::Beacon,
) {
    // process config
    let config: ModbusConfig = serde_yaml::from_str(&cfg).unwrap();
    // TODO: move to de_
    if config.proto.name != "modbus/tcp" {
        unimplemented!("protocol {}", config.proto.name);
    }
    // connect Modbus client
    let cfg = modbus::tcp::Config {
        modbus_uid: config.proto.unit,
        tcp_port: config.proto.source.port,
        tcp_connect_timeout: Some(timeout),
        tcp_read_timeout: Some(timeout),
        tcp_write_timeout: Some(timeout),
    };
    let mut client = modbus::tcp::Transport::new_with_cfg(&config.proto.source.host, cfg)
        .expect("unable to connect to source");
    // create pulls and dp list
    let mut pulls: Vec<ModbusPullData> = Vec::new();
    let mut dp_list: Vec<Vec<ModbusDataProcessInfo>> = Vec::new();
    for p in config.pull {
        let mut reg = p.reg.clone();
        let tp = reg.chars().next().unwrap();
        reg.remove(0);
        let addr = reg.safe_parse_u32();
        let register_type = match tp {
            'h' => ModbusRegisterType::Holding,
            'i' => ModbusRegisterType::Input,
            'd' => ModbusRegisterType::Discrete,
            'c' => ModbusRegisterType::Coil,
            _ => panic!("unknown register type: {}", tp),
        };
        let mut process_data_vec: Vec<ModbusDataProcessInfo> = Vec::new();
        for prc in p.process {
            let offset = prc.offset.parse_data_offset(addr);
            let tp = match offset.bit {
                Some(_) => datatypes::GenDataType::Bit,
                None => prc.r#type.parse_data_type(),
            };
            process_data_vec.push(ModbusDataProcessInfo {
                offset: offset,
                set_id: prc.set_id,
                tp: tp,
                transform: prc.transform,
            });
        }
        pulls.push(ModbusPullData {
            tp: register_type,
            addr: addr as u16,
            count: p.count,
        });
        dp_list.push(process_data_vec);
    }
    // prepare & launch processor
    // move iter to lib
    let mut pull_loop = IntervalLoop::new(interval);
    let (tx, rx) = mpsc::channel();
    // data processor
    let processor = thread::spawn(move || loop {
        let w: TaskResult = rx.recv().unwrap();
        let t = w.t;
        let v = match w.data {
            Some(v) => v,
            None => break,
        };
        let i = w.work_id.unwrap();
        for d in dp_list.get(i).unwrap() {
            match d.tp {
                GenDataType::Bit => {
                    let event = Event::new(
                        &d.set_id,
                        v.get(d.offset.offset as usize)
                            .expect(ERROR_OOB)
                            .get_bit(d.offset.bit.unwrap()),
                        &d.transform,
                        &t,
                    );
                    out.output(&event);
                }
                GenDataType::Uint16 => {
                    let event = Event::new(
                        &d.set_id,
                        *v.get(d.offset.offset as usize).expect(ERROR_OOB),
                        &d.transform,
                        &t,
                    );
                    out.output(&event);
                }
                GenDataType::Int16 => {
                    let event = Event::new(
                        &d.set_id,
                        *v.get(d.offset.offset as usize).expect(ERROR_OOB) as i16,
                        &d.transform,
                        &t,
                    );
                    out.output(&event);
                }
                GenDataType::Uint32 => {
                    let v1 = v.get(d.offset.offset as usize).expect(ERROR_OOB);
                    let v2 = v.get((d.offset.offset + 1) as usize).expect(ERROR_OOB);
                    let event =
                        Event::new(&d.set_id, *v1 as u32 * 65536 + *v2 as u32, &d.transform, &t);
                    out.output(&event);
                }
                GenDataType::Int32 => {
                    let v1 = match v.get(d.offset.offset as usize) {
                        Some(z) => z,
                        None => panic!(ERROR_OOB),
                    };
                    let v2 = match v.get((d.offset.offset + 1) as usize) {
                        Some(z) => z,
                        None => panic!(ERROR_OOB),
                    };
                    let event = Event::new(
                        &d.set_id,
                        (*v1 as u32 * 65536 + *v2 as u32) as i32,
                        &d.transform,
                        &t,
                    );
                    out.output(&event);
                }
                GenDataType::Real32 => {
                    let v1 = match v.get(d.offset.offset as usize) {
                        Some(z) => z,
                        None => panic!(ERROR_OOB),
                    };
                    let v2 = match v.get((d.offset.offset + 1) as usize) {
                        Some(z) => z,
                        None => panic!(ERROR_OOB),
                    };
                    let val: f32 = Ieee754::from_bits(*v2 as u32 * 65536 + *v1 as u32);
                    let event = Event::new(&d.set_id, val, &d.transform, &t);
                    out.output(&event);
                }
                _ => unimplemented!("data type unimplemented: {}", d.tp),
            };
        }
    });
    // pulling loop
    loop {
        for i in 0..pulls.len() {
            let call_time = EventTime::new(time_format);
            let p = pulls.get(i).unwrap();
            if verbose {
                pl::print_debug(&format!("reading registers {:?}", p));
            }
            let data = match p.tp {
                ModbusRegisterType::Holding => client.read_holding_registers(p.addr, p.count),
                ModbusRegisterType::Input => client.read_input_registers(p.addr, p.count),
                ModbusRegisterType::Coil => client.read_coils_as_u16(p.addr, p.count),
                ModbusRegisterType::Discrete => client.read_discrete_inputs_as_u16(p.addr, p.count),
            };
            tx.send(match data {
                Ok(v) => {
                    if verbose {
                        pl::print_debug(&format!("{:?}", v));
                    }
                    TaskResult {
                        data: Some(v),
                        work_id: Some(i),
                        t: call_time,
                    }
                }
                Err(err) => panic!("{}", err),
            })
            .unwrap();
        }
        if !inloop {
            break;
        }
        beacon.ping();
        pull_loop.sleep()
    }
    terminate_processor!(processor, tx);
    client.close().expect("client disconnected with an error");
}
