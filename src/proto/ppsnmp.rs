use pl::IntervalLoop;
use serde::{Deserialize, Deserializer};
use std::ffi;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use pl::datatypes;
use pl::datatypes::{Event, EventTime, GenDataType, GenDataTypeParse, ParseData};

use snmp;

const DEFAULT_SNMP_PORT: u16 = 161;

#[derive(Deserialize)]
struct SNMPConfig {
    proto: SNMPProto,
    pull: Vec<SNMPPull>,
}

fn get_default_version() -> u8 {
    return 2;
}

define_de_source!(DEFAULT_SNMP_PORT);

#[derive(Deserialize)]
struct SNMPProto {
    name: String,
    #[serde(deserialize_with = "de_source")]
    source: HostPort,
    version: u8,
}

fn get_default_non_repeat() -> u32 {
    return 0;
}

fn get_default_max_repeat() -> u32 {
    return 1;
}

#[derive(Deserialize)]
struct SNMPPull {
    #[serde(alias = "oid")]
    oids: Vec<String>,
    #[serde(alias = "non-repeat", default = "get_default_non_repeat")]
    non_repeat: u32,
    #[serde(alias = "max-repeat", default = "get_default_max_repeat")]
    max_repeat: u32,
    process: Vec<SNMPProcess>,
}

fn get_default_set_id() -> Option<String> {
    return None;
}

#[derive(Deserialize)]
struct SNMPProcess {
    offset: String,
    #[serde(alias = "set-id", default = "get_default_set_id")]
    set_id: Option<String>,
    #[serde(default = "datatypes::empty_transform_task")]
    transform: datatypes::EventTransformList,
}

// TODO: move some fields to de_
struct SNMPPullData {
    oids: Vec<Vec<u32>>,
    non_repeat: u32,
    max_repeat: u32,
}

// TODO: move some fields to de_
struct SNMPDataProcessInfo {
    set_id: Option<String>,
    transform: datatypes::EventTransformList,
}

struct TaskResult<'a> {
    data: Option<snmp::SnmpPdu<'a>>,
    work_id: Option<usize>,
    t: datatypes::EventTime,
}

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
    let config: SNMPConfig = serde_yaml::from_str(&cfg).unwrap();
    // TODO: move to de_
    if config.proto.version != 2 {
        unimplemented!("SNMP version {}", config.proto.version);
    }
    let mut pulls: Vec<SNMPPullData> = Vec::new();
    let mut dp_list: Vec<Vec<SNMPDataProcessInfo>> = Vec::new();
    // TODO implement ignore list for bulk requests
    for p in config.pull {
        let mut process_data_vec: Vec<SNMPDataProcessInfo> = Vec::new();
        for prc in p.process {
            process_data_vec.push(SNMPDataProcessInfo {
                set_id: prc.set_id,
                transform: prc.transform,
            });
        }
        let mut oids: Vec<Vec<u32>> = Vec::new();
        for oid in p.oids {
            oids.push(oid.split("/").map(|s| s.parse::<u32>().unwrap()).collect());
        }
        pulls.push(SNMPPullData {
            oids: oids,
            non_repeat: p.non_repeat,
            max_repeat: p.max_repeat,
        });
        dp_list.push(process_data_vec);
    }
    // prepare & launch processor
    // move iter to lib
    let mut pull_loop = IntervalLoop::new(interval);
    let (tx, rx) = mpsc::channel();
    let mut sess = snmp::SyncSession::new(
        format!("{}:{}", config.proto.source.host, config.proto.source.port),
        "public".as_bytes(),
        Some(timeout),
        0,
    )
    .unwrap();
    // data processor
    let processor = thread::spawn(move || loop {
        let w: TaskResult = rx.recv().unwrap();
        let t = w.t;
        let snmp_result = match w.data {
            Some(v) => v,
            None => break,
        };
        let i = w.work_id.unwrap();
        for d in dp_list.get(i).unwrap() {}
    });
    // pulling loop
    loop {
        for work_id in 0..pulls.len() {
            let call_time = EventTime::new(time_format);
            let p = pulls.get(work_id).unwrap();
            let response: snmp::SnmpPdu;
            //let mut response;
            if p.oids.len() > 1 {
                let mut z = vec![];
                for o in &p.oids {
                    z.push(o.as_slice());
                }
                let response = sess
                    .getbulk(z.as_slice(), p.non_repeat, p.max_repeat)
                    .expect("SNMP GET error");
            tx.send(TaskResult {
                data: Some(response),
                work_id: Some(work_id),
                t: call_time,
            })
            .unwrap();
            } else {
                let response = sess
                    .getnext(p.oids.get(0).unwrap().as_slice())
                    .expect("SNMP GETBULK error");
            tx.send(TaskResult {
                data: Some(response),
                work_id: Some(work_id),
                t: call_time,
            })
            .unwrap();
            }
        }
        if !inloop {
            break;
        }
        beacon.ping();
        pull_loop.sleep();
    }
    terminate_processor!(processor, tx);
}
