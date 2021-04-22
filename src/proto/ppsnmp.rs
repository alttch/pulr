use pl::IntervalLoop;
use serde::{Deserialize, Deserializer};
use std::collections::HashMap;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use pl::datatypes;

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

fn get_default_community() -> String {
    return "public".to_string();
}

define_de_source!(DEFAULT_SNMP_PORT);

#[derive(Deserialize)]
struct SNMPProto {
    //name: String,
    #[serde(deserialize_with = "de_source")]
    source: HostPort,
    #[serde(default = "get_default_version")]
    version: u8,
    #[serde(default = "get_default_community")]
    community: String,
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
    oid: String,
    #[serde(alias = "set-id", default = "get_default_set_id")]
    set_id: Option<String>,
    #[serde(default = "datatypes::empty_transform_task")]
    transform: datatypes::EventTransformList,
}

// TODO: move some fields to de_
struct SNMPPullData {
    label: String,
    oids: Vec<Vec<u32>>,
    non_repeat: u32,
    max_repeat: u32,
    bulk: bool,
}

// TODO: move some fields to de_
struct SNMPDataProcessInfo {
    oid: String,
    set_id: Option<String>,
    transform: datatypes::EventTransformList,
}

#[derive(Debug)]
enum SNMPValue {
    SBoolean(u8),
    SUint32(u32),
    SInt64(i64),
    SUint64(u64),
    SStr(String),
    SNull,
}

fn parse_snmp_val(value: snmp::Value) -> SNMPValue {
    use snmp::Value::*;
    use SNMPValue::*;
    return match value {
        Boolean(v) => {
            if v {
                SBoolean(1)
            } else {
                SBoolean(0)
            }
        }
        Integer(v) => SInt64(v),
        OctetString(v) => SStr(String::from_utf8_lossy(v).to_string()),
        ObjectIdentifier(ref v) => SStr(v.to_string()),
        IpAddress(v) => SStr(format!("{}.{}.{}.{}", v[0], v[1], v[2], v[3])),
        Counter32(v) => SUint32(v),
        Unsigned32(v) => SUint32(v),
        Timeticks(v) => SUint32(v),
        Counter64(v) => SUint64(v),
        _ => SNull,
    };
}

fn prepare_oid(oid: &String) -> String {
    let mut res = oid.to_owned();
    if res.chars().next().unwrap() == '.' {
        res.remove(0);
    } else if res.starts_with("iso.") {
        res = "1.".to_string() + &res[4..];
    }
    return res;
}

define_task_result!(HashMap<String, SNMPValue>);

pub fn run(
    inloop: bool,
    verbose: bool,
    verbose_warnings: bool,
    cfg: String,
    timeout: Duration,
    interval: Duration,
    resend_interval: Option<Duration>,
    core: pl::Core,
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
    for p in config.pull {
        let mut process_data_vec: Vec<SNMPDataProcessInfo> = Vec::new();
        for prc in p.process {
            process_data_vec.push(SNMPDataProcessInfo {
                oid: prepare_oid(&prc.oid),
                set_id: prc.set_id,
                transform: prc.transform,
            });
        }
        let mut oids: Vec<Vec<u32>> = Vec::new();
        for oid in &p.oids {
            oids.push(
                prepare_oid(oid)
                    .split(".")
                    .map(|s| s.parse::<u32>().unwrap())
                    .collect(),
            );
        }
        let bulk: bool = p.max_repeat > 1 || oids.len() > 1;
        pulls.push(SNMPPullData {
            label: format!("{:?}", p.oids),
            oids,
            non_repeat: p.non_repeat,
            max_repeat: p.max_repeat,
            bulk,
        });
        dp_list.push(process_data_vec);
    }
    // prepare & launch processor
    let mut pull_loop = IntervalLoop::new(interval);
    let mut sess = snmp::SyncSession::new(
        format!("{}:{}", config.proto.source.host, config.proto.source.port),
        config.proto.community.as_bytes(),
        Some(timeout),
        0,
    )
    .unwrap();
    let (tx, rx) = mpsc::channel();
    // data processor
    macro_rules! debug_snmp_result {
        ($v:path) => {
            if verbose {
                pl::print_debug(&"SNMP result\n--------------".to_string());
                for (k, v) in $v.iter() {
                    pl::print_debug(&format!("{} = '{:?}'", k, v))
                }
                pl::print_debug(&"--------------".to_string());
            }
        };
    }
    let processor = thread::spawn(move || loop {
        let w: TaskResult = rx.recv().unwrap();
        let t = w.t;
        let snmp_result = match w.data {
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
            macro_rules! process_snmp_result {
                ($i:path, $v:path) => {
                    let event = core.create_event(&$i, *$v, &d.transform, &t);
                    core.output(&event);
                };
            }
            use SNMPValue::*;
            match snmp_result.get(&d.oid) {
                Some(v) => {
                    let id = match &d.set_id {
                        Some(v) => v,
                        None => &d.oid,
                    };
                    match v {
                        SBoolean(v) => {
                            process_snmp_result!(id, v);
                        }
                        SUint32(v) => {
                            process_snmp_result!(id, v);
                        }
                        SInt64(v) => {
                            process_snmp_result!(id, v);
                        }
                        SUint64(v) => {
                            process_snmp_result!(id, v);
                        }
                        SStr(v) => {
                            let event = core.create_event(&id, v.to_owned(), &d.transform, &t);
                            core.output(&event);
                        }
                        SNull => {
                            pl::print_debug(&format!("Unsupported datatype value of {}", d.oid));
                        }
                    }
                }
                None => {
                    if verbose {
                        pl::print_debug(&format!("No data of {}", d.oid));
                    }
                }
            }
        }
    });
    // pulling loop
    let mut resend_time = match resend_interval {
        Some(v) => Some(Instant::now() + v),
        None => None,
    };
    let mut pull_log: datatypes::PullLog = datatypes::PullLog::new();
    loop {
        if verbose_warnings {
            pull_log.clear();
        }
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
        for work_id in 0..pulls.len() {
            let call_time = core.create_event_time();
            let p = pulls.get(work_id).unwrap();
            let mut pull_log_entry = match verbose_warnings {
                true => Some(datatypes::PullLogEntry::new(&p.label)),
                false => None,
            };
            // TODO: move slices to prepare stage
            if p.bulk {
                if verbose {
                    pl::print_debug(&format!("SNMP GETBULK {:?}", p.oids));
                }
                let mut z = vec![];
                for o in &p.oids {
                    z.push(o.as_slice());
                }
                let response = sess
                    .getbulk(z.as_slice(), p.non_repeat, p.max_repeat)
                    .expect("SNMP GETBULK error");
                let mut result: HashMap<String, SNMPValue> = HashMap::new();
                for (name, val) in response.varbinds {
                    result.insert(name.to_string(), parse_snmp_val(val));
                }
                debug_snmp_result!(result);
                log_pulled!(pull_log_entry);
                tx.send(TaskResult {
                    data: Some(result),
                    work_id: Some(work_id),
                    t: call_time,
                    cmd: TaskCmd::Process,
                })
                .unwrap();
            } else {
                let o = p.oids.get(0).unwrap();
                if verbose {
                    pl::print_debug(&format!("SNMP GET {:?}", o));
                }
                let mut response = sess
                    .getnext(o.as_slice())
                    .expect(&format!("SNMP GET error {:?}", o));
                let (name, val) = response
                    .varbinds
                    .next()
                    .expect(&format!("SNMP GET parse error {:?}", o));
                let mut result: HashMap<String, SNMPValue> = HashMap::new();
                result.insert(name.to_string(), parse_snmp_val(val));
                debug_snmp_result!(result);
                log_pulled!(pull_log_entry);
                tx.send(TaskResult {
                    data: Some(result),
                    work_id: Some(work_id),
                    t: call_time,
                    cmd: TaskCmd::Process,
                })
                .unwrap();
            }
            if verbose_warnings {
                pull_log.push_entry(pull_log_entry.unwrap())
            };
        }
        if !inloop {
            break;
        }
        sleep_loop!(pull_loop, pull_log, verbose_warnings);
        beacon.ping();
    }
    terminate_processor!(processor, tx);
}
