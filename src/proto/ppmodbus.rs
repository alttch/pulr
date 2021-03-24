use serde::{Deserialize, Deserializer};

use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream, UdpSocket};

use std::sync::mpsc;

use ieee754::Ieee754;

use std::thread;
use std::time::{Duration, Instant};

use pl::tools::GetBit;
use pl::IntervalLoop;

use datatypes::{GenDataType, GenDataTypeParse, ParseData};
use pl::datatypes;

use rmodbus::{client::ModbusRequest, guess_response_frame_len, ModbusFrameBuf, ModbusProto};

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
    proto: ModbusProtocol,
    pull: Vec<ModbusPull>,
}

define_de_source!(DEFAULT_MODBUS_PORT);

fn get_default_unit() -> u8 {
    return 0;
}

#[derive(Deserialize)]
struct ModbusProtocol {
    name: String,
    #[serde(deserialize_with = "de_source")]
    source: HostPort,
    #[serde(default = "get_default_unit")]
    unit: u8,
}

#[derive(Deserialize)]
struct ModbusPull {
    reg: String,
    count: u16,
    process: Vec<ModbusProcess>,
    #[serde(default = "get_default_unit")]
    unit: u8,
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

// need to read coil data as Vec<u16>
trait ModbusDataAsU16 {
    fn parse_bool_as_u16(
        &mut self,
        response: &[u8],
        result: &mut Vec<u16>,
    ) -> Result<(), rmodbus::ErrorKind>;
}

impl ModbusDataAsU16 for ModbusRequest {
    fn parse_bool_as_u16(
        &mut self,
        response: &[u8],
        result: &mut Vec<u16>,
    ) -> Result<(), rmodbus::ErrorKind> {
        let mut result_bool = Vec::new();
        let r = self.parse_bool(response, &mut result_bool);
        if r.is_err() {
            return Err(r.err().unwrap());
        }
        for v in result_bool {
            result.push(match v {
                true => 1,
                false => 0,
            });
        }
        Ok(())
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
    unit: u8,
}

define_task_result!(Vec<u16>);

#[derive(Debug, Eq, PartialEq)]
enum ErrorKind {
    ServerError,
    UdpBindError,
}

struct TcpClient {
    stream: std::net::TcpStream,
    tr_id: u16,
}

struct UdpClient {
    socket: std::net::UdpSocket,
    tr_id: u16,
    target: SocketAddr,
}

macro_rules! prepare_mreq {
    ($unit_id:expr) => {
        ModbusRequest::new($unit_id, ModbusProto::TcpUdp);
    };
}

macro_rules! incr_tr_id {
    ($self:expr) => {
        $self.tr_id = match $self.tr_id {
            u16::MAX => 0,
            _ => $self.tr_id + 1,
        }
    };
}

impl TcpClient {
    fn new(host: &str, port: u16, timeout: Duration) -> Result<Self, ErrorKind> {
        let addr: SocketAddr = format!("{}:{}", host, port).parse().unwrap();
        let stream = TcpStream::connect_timeout(&addr, timeout);
        if stream.is_err() {
            Err(ErrorKind::ServerError)
        } else {
            {
                let s = stream.unwrap();
                s.set_read_timeout(Some(timeout)).unwrap();
                s.set_write_timeout(Some(timeout)).unwrap();
                Ok(Self {
                    stream: s,
                    tr_id: 1,
                })
            }
        }
    }
}

impl UdpClient {
    fn new(host: &str, port: u16, timeout: Duration) -> Result<Self, ErrorKind> {
        let mut socket: Option<UdpSocket> = None;
        let target: SocketAddr = format!("{}:{}", host, port).parse().unwrap();
        for i in 20000..60000 {
            let bind_addr: SocketAddr = format!("0.0.0.0:{}", i).parse().unwrap();
            match UdpSocket::bind(bind_addr) {
                Ok(v) => {
                    socket = Some(v);
                    break;
                }
                Err(_) => {}
            }
        }
        if socket.is_none() {
            Err(ErrorKind::UdpBindError)
        } else {
            {
                let s = socket.unwrap();
                s.set_read_timeout(Some(timeout)).unwrap();
                s.set_write_timeout(Some(timeout)).unwrap();
                Ok(Self {
                    socket: s,
                    tr_id: 1,
                    target: target,
                })
            }
        }
    }
}

trait ModbusNetworkClient {
    fn process_request(&mut self, request: &[u8]) -> Result<Vec<u8>, rmodbus::ErrorKind>;
    fn prepare(&mut self);
}

impl ModbusNetworkClient for TcpClient {
    fn process_request(&mut self, request: &[u8]) -> Result<Vec<u8>, rmodbus::ErrorKind> {
        if self.stream.write(&request).is_err() {
            return Err(rmodbus::ErrorKind::CommunicationError);
        }
        let mut buf = [0u8; 6];
        if self.stream.read_exact(&mut buf).is_err() {
            return Err(rmodbus::ErrorKind::CommunicationError);
        }
        let mut response = Vec::new();
        response.extend_from_slice(&buf);
        let len = guess_response_frame_len(&buf, ModbusProto::TcpUdp).unwrap();
        if len > 6 {
            let mut rest = vec![0u8; (len - 6) as usize];
            if self.stream.read_exact(&mut rest).is_err() {
                return Err(rmodbus::ErrorKind::CommunicationError);
            }
            response.extend(rest);
        }
        Ok(response)
    }
    fn prepare(&mut self) {
        incr_tr_id!(self);
    }
}

impl ModbusNetworkClient for UdpClient {
    fn process_request(&mut self, request: &[u8]) -> Result<Vec<u8>, rmodbus::ErrorKind> {
        let len = match self.socket.send_to(request, self.target) {
            Ok(v) => v,
            Err(_) => return Err(rmodbus::ErrorKind::CommunicationError),
        };
        if len != request.len() {
            return Err(rmodbus::ErrorKind::CommunicationError);
        }
        let mut buf: ModbusFrameBuf = [0; 256];
        let (n, _addr) = match self.socket.recv_from(&mut buf) {
            Ok(v) => v,
            Err(_) => {
                return Err(rmodbus::ErrorKind::CommunicationError);
            }
        };
        let mut response = Vec::new();
        response.extend_from_slice(&buf[..n]);
        Ok(response)
    }
    fn prepare(&mut self) {
        incr_tr_id!(self);
    }
}

macro_rules! prepare_client {
    ($client:expr) => {
        match $client {
            ModbusClient::Tcp(c) => c.prepare(),
            ModbusClient::Udp(c) => c.prepare(),
        };
    };
}

macro_rules! process_request {
    ($client:expr, $request:expr) => {
        match $client {
            ModbusClient::Tcp(c) => c.process_request($request),
            ModbusClient::Udp(c) => c.process_request($request),
        }
    };
}

fn read_coils(
    client: &mut ModbusClient,
    unit_id: u8,
    reg: u16,
    count: u16,
) -> Result<Vec<u16>, rmodbus::ErrorKind> {
    prepare_client!(client);
    let mut mreq = prepare_mreq!(unit_id);
    let mut request = Vec::new();
    match mreq.generate_get_coils(reg, count, &mut request) {
        Ok(_) => {}
        Err(e) => return Err(e),
    }
    let response = match process_request!(client, &request) {
        Ok(v) => v,
        Err(e) => {
            return Err(e);
        }
    };
    let mut result = Vec::new();
    match mreq.parse_bool_as_u16(&response, &mut result) {
        Ok(_) => Ok(result),
        Err(e) => Err(e),
    }
}
fn read_discretes(
    client: &mut ModbusClient,
    unit_id: u8,
    reg: u16,
    count: u16,
) -> Result<Vec<u16>, rmodbus::ErrorKind> {
    prepare_client!(client);
    let mut mreq = prepare_mreq!(unit_id);
    let mut request = Vec::new();
    match mreq.generate_get_discretes(reg, count, &mut request) {
        Ok(_) => {}
        Err(e) => return Err(e),
    }
    let response = match process_request!(client, &request) {
        Ok(v) => v,
        Err(e) => {
            return Err(e);
        }
    };
    let mut result = Vec::new();
    match mreq.parse_bool_as_u16(&response, &mut result) {
        Ok(_) => Ok(result),
        Err(e) => Err(e),
    }
}
fn read_inputs(
    client: &mut ModbusClient,
    unit_id: u8,
    reg: u16,
    count: u16,
) -> Result<Vec<u16>, rmodbus::ErrorKind> {
    prepare_client!(client);
    let mut mreq = prepare_mreq!(unit_id);
    let mut request = Vec::new();
    match mreq.generate_get_inputs(reg, count, &mut request) {
        Ok(_) => {}
        Err(e) => return Err(e),
    }
    let response = match process_request!(client, &request) {
        Ok(v) => v,
        Err(e) => {
            return Err(e);
        }
    };
    let mut result = Vec::new();
    match mreq.parse_u16(&response, &mut result) {
        Ok(_) => Ok(result),
        Err(e) => Err(e),
    }
}
fn read_holdings(
    client: &mut ModbusClient,
    unit_id: u8,
    reg: u16,
    count: u16,
) -> Result<Vec<u16>, rmodbus::ErrorKind> {
    prepare_client!(client);
    let mut mreq = prepare_mreq!(unit_id);
    let mut request = Vec::new();
    match mreq.generate_get_holdings(reg, count, &mut request) {
        Ok(_) => {}
        Err(e) => return Err(e),
    }
    let response = match process_request!(client, &request) {
        Ok(v) => v,
        Err(e) => {
            return Err(e);
        }
    };
    let mut result = Vec::new();
    match mreq.parse_u16(&response, &mut result) {
        Ok(_) => Ok(result),
        Err(e) => Err(e),
    }
}

enum ModbusClient {
    Tcp(TcpClient),
    Udp(UdpClient),
}

pub fn run(
    inloop: bool,
    verbose: bool,
    cfg: String,
    timeout: Duration,
    interval: Duration,
    resend_interval: Option<Duration>,
    core: pl::Core,
    beacon: &mut pl::Beacon,
) {
    // process config
    let config: ModbusConfig = serde_yaml::from_str(&cfg).unwrap();
    // connect Modbus client
    let mut client: ModbusClient = match config.proto.name.as_str() {
        "modbus/tcp" => ModbusClient::Tcp(
            TcpClient::new(&config.proto.source.host, config.proto.source.port, timeout)
                .expect("unable to connect to server"),
        ),
        "modbus/udp" => ModbusClient::Udp(
            UdpClient::new(&config.proto.source.host, config.proto.source.port, timeout).unwrap(),
        ),
        _ => {
            unimplemented!("protocol {}", config.proto.name);
        }
    };
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
                offset,
                set_id: prc.set_id,
                tp,
                transform: prc.transform,
            });
        }
        let mut unit = p.unit;
        if unit == 0 {
            unit = config.proto.unit;
        }
        if unit == 0 {
            panic!("Modbus unit not specified, neither in pull config, nor default");
        }
        pulls.push(ModbusPullData {
            tp: register_type,
            addr: addr as u16,
            count: p.count,
            unit,
        });
        dp_list.push(process_data_vec);
    }
    // prepare & launch processor
    let mut pull_loop = IntervalLoop::new(interval);
    let (tx, rx) = mpsc::channel();
    // data processor
    let processor = thread::spawn(move || loop {
        let w: TaskResult = rx.recv().unwrap();
        let t = w.t;
        let v = match w.data {
            Some(v) => v,
            None => match w.cmd {
                TaskCmd::ClearCache => {
                    core.clear_event_cache();
                    continue;
                }
                TaskCmd::Terminate => break,
                _ => continue,
            },
        };
        let i = w.work_id.unwrap();
        for d in dp_list.get(i).unwrap() {
            match d.tp {
                GenDataType::Bit => {
                    let event = core.create_event(
                        &d.set_id,
                        v.get(d.offset.offset as usize)
                            .expect(ERROR_OOB)
                            .get_bit(d.offset.bit.unwrap()),
                        &d.transform,
                        &t,
                    );
                    core.output(&event);
                }
                GenDataType::Uint16 => {
                    let event = core.create_event(
                        &d.set_id,
                        *v.get(d.offset.offset as usize).expect(ERROR_OOB),
                        &d.transform,
                        &t,
                    );
                    core.output(&event);
                }
                GenDataType::Int16 => {
                    let event = core.create_event(
                        &d.set_id,
                        *v.get(d.offset.offset as usize).expect(ERROR_OOB) as i16,
                        &d.transform,
                        &t,
                    );
                    core.output(&event);
                }
                GenDataType::Uint32 => {
                    let v1 = v.get(d.offset.offset as usize).expect(ERROR_OOB);
                    let v2 = v.get((d.offset.offset + 1) as usize).expect(ERROR_OOB);
                    let event = core.create_event(
                        &d.set_id,
                        *v1 as u32 * 65536 + *v2 as u32,
                        &d.transform,
                        &t,
                    );
                    core.output(&event);
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
                    let event = core.create_event(
                        &d.set_id,
                        (*v1 as u32 * 65536 + *v2 as u32) as i32,
                        &d.transform,
                        &t,
                    );
                    core.output(&event);
                }
                GenDataType::Uint64 => {
                    let v1 = *v.get(d.offset.offset as usize).expect(ERROR_OOB) as u64;
                    let v2 = *v.get((d.offset.offset + 1) as usize).expect(ERROR_OOB) as u64;
                    let v3 = *v.get((d.offset.offset + 2) as usize).expect(ERROR_OOB) as u64;
                    let v4 = *v.get((d.offset.offset + 3) as usize).expect(ERROR_OOB) as u64;
                    let value = u64::from_be_bytes([
                        (v1 >> 8) as u8,
                        v1 as u8,
                        (v2 >> 8) as u8,
                        v2 as u8,
                        (v3 >> 8) as u8,
                        v3 as u8,
                        (v4 >> 8) as u8,
                        v4 as u8,
                    ]);
                    let event = core.create_event(&d.set_id, value, &d.transform, &t);
                    core.output(&event);
                }
                GenDataType::Int64 => {
                    let v1 = *v.get(d.offset.offset as usize).expect(ERROR_OOB) as u64;
                    let v2 = *v.get((d.offset.offset + 1) as usize).expect(ERROR_OOB) as u64;
                    let v3 = *v.get((d.offset.offset + 2) as usize).expect(ERROR_OOB) as u64;
                    let v4 = *v.get((d.offset.offset + 3) as usize).expect(ERROR_OOB) as u64;
                    let value = u64::from_be_bytes([
                        (v1 >> 8) as u8,
                        v1 as u8,
                        (v2 >> 8) as u8,
                        v2 as u8,
                        (v3 >> 8) as u8,
                        v3 as u8,
                        (v4 >> 8) as u8,
                        v4 as u8,
                    ]);
                    let event = core.create_event(&d.set_id, value as i64, &d.transform, &t);
                    core.output(&event);
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
                    let event = core.create_event(&d.set_id, val, &d.transform, &t);
                    core.output(&event);
                }
                _ => unimplemented!("data type unimplemented: {}", d.tp),
            };
        }
    });
    // pulling loop
    let mut resend_time = match resend_interval {
        Some(v) => Some(Instant::now() + v),
        None => None,
    };
    loop {
        match resend_time {
            Some(ref mut v) => {
                let t = Instant::now();
                if t > *v {
                    while t > *v {
                        *v += resend_interval.unwrap();
                    }
                    clear_processor_cache!(processor, tx);
                }
            }
            None => {}
        };
        for i in 0..pulls.len() {
            let call_time = core.create_event_time();
            let p = pulls.get(i).unwrap();
            if verbose {
                pl::print_debug(&format!("reading registers {:?}", p));
            }
            let data = match p.tp {
                ModbusRegisterType::Holding => read_holdings(&mut client, p.unit, p.addr, p.count),
                ModbusRegisterType::Input => read_inputs(&mut client, p.unit, p.addr, p.count),
                ModbusRegisterType::Coil => read_coils(&mut client, p.unit, p.addr, p.count),
                ModbusRegisterType::Discrete => {
                    read_discretes(&mut client, p.unit, p.addr, p.count)
                }
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
                        cmd: TaskCmd::Process,
                    }
                }
                Err(err) => panic!("{:?}", err),
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
}
