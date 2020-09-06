use pl::IntervalLoop;
use serde::{Deserialize, Deserializer};
use std::collections::HashMap;
use std::ffi;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration};

use pl::datatypes;
use pl::datatypes::{Event, EventTime, GenDataType, GenDataTypeParse, ParseData};

use plctag;

const DEFAULT_ENIP_PORT: u16 = 44818;
const PLC_SLEEP_STEP: u32 = 10_000_000;


#[derive(Deserialize)]
struct EnIpConfig {
    proto: EnIpProto,
    pull: Vec<EnIpPull>,
}

define_de_source!(DEFAULT_ENIP_PORT);

#[derive(Deserialize)]
struct EnIpProto {
    name: String,
    #[serde(deserialize_with = "de_source")]
    source: HostPort,
    path: String,
    cpu: String,
}

fn get_default_size() -> u32 {
    return 1;
}

fn get_default_count() -> Option<u32> {
    return None;
}

#[derive(Deserialize)]
struct EnIpPull {
    tag: String,
    #[serde(default = "get_default_size")]
    size: u32,
    #[serde(default = "get_default_count")]
    count: Option<u32>,
    process: Vec<EnIpProcess>,
}

#[derive(Deserialize)]
struct EnIpProcess {
    offset: String,
    #[serde(alias = "type")]
    r#type: String,
    #[serde(alias = "set-id")]
    set_id: String,
    #[serde(default = "datatypes::empty_transform_task")]
    transform: datatypes::EventTransformList,
}

// TODO: move some fields to de_
struct EnIpPullData {
    path: String,
    path_hash: u64,
}

// TODO: move some fields to de_
struct EnIpDataProcessInfo {
    offset: u32,
    tp: datatypes::GenDataType,
    set_id: String,
    transform: datatypes::EventTransformList,
}

define_task_result!(i32);

pub fn run(
    inloop: bool,
    cfg: String,
    timeout: Duration,
    interval: Duration,
    time_format: datatypes::TimeFormat,
    out: pl::Output,
    beacon: &mut pl::Beacon,
) {
    // process config
    let config: EnIpConfig = serde_yaml::from_str(&cfg).unwrap();
    // TODO: move to de_
    if config.proto.name != "enip/ab_eip" {
        unimplemented!("protocol {}", config.proto.name);
    }
    let proto_id = "ab_eip";
    let plc_path = format!(
        "protocol={}&gateway={}:{}&path={}&cpu={}",
        proto_id,
        config.proto.source.host,
        config.proto.source.port,
        config.proto.path,
        config.proto.cpu
    );
    let mut pulls: Vec<EnIpPullData> = Vec::new();
    let mut dp_list: Vec<Vec<EnIpDataProcessInfo>> = Vec::new();
    let mut active_tags: HashMap<u64, i32> = HashMap::new();
    let plc_timeout = timeout.as_millis() as i32;
    let plc_sleep_step = Duration::new(0, PLC_SLEEP_STEP);
    for p in config.pull {
        let mut process_data_vec: Vec<EnIpDataProcessInfo> = Vec::new();
        for prc in p.process {
            let offset = prc.offset.safe_parse_u32();
            let tp = prc.r#type.parse_data_type();
            process_data_vec.push(EnIpDataProcessInfo {
                offset: offset,
                set_id: prc.set_id,
                tp: tp,
                transform: prc.transform,
            });
        }
        let path = format!(
            "{}&elem_size={}{}&name={}",
            plc_path,
            p.size,
            match p.count {
                Some(v) => format!("&elem_count={}", v),
                None => "".to_owned(),
            },
            p.tag
        );
        let path_hash = datatypes::calculate_hash(&path);
        pulls.push(EnIpPullData {
            path: path,
            path_hash: path_hash,
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
        let tag_id = match w.data {
            Some(v) => v,
            None => break,
        };
        let i = w.work_id.unwrap();
        for d in dp_list.get(i).unwrap() {
            macro_rules! process_tag {
                ($fn:path) => {
                    unsafe {
                        let event =
                            Event::new(&d.set_id, $fn(tag_id, d.offset as i32), &d.transform, &t);
                        out.output(&event);
                    }
                };
            }
            match d.tp {
                GenDataType::Uint8 => {
                    process_tag!(plctag::plc_tag_get_uint8);
                }
                GenDataType::Int8 => {
                    process_tag!(plctag::plc_tag_get_int8);
                }
                GenDataType::Uint16 => {
                    process_tag!(plctag::plc_tag_get_uint16);
                }
                GenDataType::Int16 => {
                    process_tag!(plctag::plc_tag_get_int16);
                }
                GenDataType::Uint32 => {
                    process_tag!(plctag::plc_tag_get_uint32);
                }
                GenDataType::Int32 => {
                    process_tag!(plctag::plc_tag_get_int32);
                }
                GenDataType::Uint64 => {
                    process_tag!(plctag::plc_tag_get_uint64);
                }
                GenDataType::Int64 => {
                    process_tag!(plctag::plc_tag_get_int64);
                }
                GenDataType::Real32 => {
                    process_tag!(plctag::plc_tag_get_float32);
                }
                GenDataType::Real64 => {
                    process_tag!(plctag::plc_tag_get_float64);
                }
                _ => unimplemented!("data type unimplemented: {}", d.tp),
            }
        }
    });
    // pulling loop
    loop {
        for work_id in 0..pulls.len() {
            let call_time = EventTime::new(time_format);
            let p = pulls.get(work_id).unwrap();
            let tag_id = match active_tags.get(&p.path_hash) {
                Some(v) => *v,
                None => unsafe {
                    let path = ffi::CString::new(p.path.to_owned()).unwrap();
                    let tag_id = plctag::plc_tag_create(path.as_ptr(), plc_timeout);
                    if tag_id < 0 {
                        panic!("{} error {}", p.path, tag_id);
                    }
                    loop {
                        let rc = plctag::plc_tag_status(tag_id);
                        if rc == plctag::PLCTAG_STATUS_PENDING {
                            thread::sleep(plc_sleep_step);
                            continue;
                        } else if rc != plctag::PLCTAG_STATUS_OK {
                            panic!("{} status error {}", p.path, rc);
                        }
                        break;
                    }
                    active_tags.insert(p.path_hash, tag_id);
                    tag_id
                },
            };
            unsafe {
                let rc = plctag::plc_tag_read(tag_id, plc_timeout);
                if rc != plctag::PLCTAG_STATUS_OK {
                    panic!("{} read error {}", p.path, rc);
                }
            }
            tx.send(TaskResult {
                data: Some(tag_id),
                work_id: Some(work_id),
                t: call_time,
            })
            .unwrap();
        }
        if !inloop {
            break;
        }
        beacon.ping();
        pull_loop.sleep();
    }
    terminate_processor!(processor, tx);
}
