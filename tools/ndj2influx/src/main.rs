use argparse::{ArgumentParser, Store, StoreTrue};
use chrono::DateTime;
use colored::Colorize;
use std::collections::HashMap;
use std::env;
use std::io::{self, BufRead};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};


const VERSION: &str = "0.0.3";

fn parse_timestamp(map: &HashMap<String, serde_json::Value>, tcol: &str) -> i64 {
    if let "@" = tcol {
        SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("Time went backwards")
        .as_nanos() as i64
    } else {
        {
                use serde_json::value::Value::{Number, String};
                let t = map
                    .get(tcol)
                    .unwrap_or_else(|| panic!("Time column not found: {}", tcol));
                match t {
                    Number(v) => (v.as_f64().unwrap() * 1_000_000_000.0) as i64,
                    String(v) => {
                        let dt = DateTime::parse_from_rfc3339(&v).unwrap();
                        dt.timestamp_nanos()
                    }
                    _ => panic!("time is in wrong format: {:?}", t),
                }
            }
    }
}

fn parse_value(val: &serde_json::Value, verbose: bool) -> Option<f64> {
    use serde_json::value::Value::Number;
    if let Number(v) = val {
        v.as_f64()
    } else {
        if verbose {
            eprintln!(
                "{}",
                format!("skipping non-numeric value {:?}", val)
                    .white()
                    .dimmed()
            );
        }
        None
    }
}

fn parse_basecol(map: &HashMap<String, serde_json::Value>, basecol: &str) -> String {
    use serde_json::value::Value::String;
    let value = map
        .get(basecol)
        .unwrap_or_else(|| panic!("base column not found: {}", basecol));
    match value {
        String(v) => v.to_owned(),
        _ => panic!("base in wrong format: {:?}", value),
    }
}

fn parse_metrics(
    map: &HashMap<String, serde_json::Value>,
    metric_col: &str,
    value_col: &str,
    time_col: &str,
    basecol: &str,
    verbose: bool,
) -> HashMap<String, f64> {
    let mut data: HashMap<String, f64> = HashMap::new();
    if metric_col.is_empty() {
        for (metric, val) in map {
            if metric != time_col && metric != basecol {
                if let Some(v) = parse_value(val, verbose) {
                    let _ = data.insert(metric.to_owned(), v);
                }
            }
        }
    } else {
        use serde_json::value::Value::String;
        let mj = map
            .get(metric_col)
            .unwrap_or_else(|| panic!("metric col not found: {}", metric_col));
        let metric = match mj {
            String(v) => v,
            _ => panic!("metric ID in wrong format"),
        };
        let val = map
            .get(value_col)
            .unwrap_or_else(|| panic!("value col not found: {}", value_col));
        if let Some(v) = parse_value(val, verbose) {
            let _ = data.insert(metric.to_owned(), v);
        }
    }
    data
}

fn main() {
    #[cfg(windows)]
    colored::control::set_override(false);

    let mut basecol = String::new();
    let mut url = String::new();
    let mut database = String::new();
    let mut bucket = String::new();
    let mut auth = match env::var("INFLUXDB_AUTH") {
        Ok(val) => val,
        Err(_) => String::new(),
    };
    let mut verbose: bool = false;
    let mut time_col = "time".to_owned();
    let mut metric_col = String::new();
    let mut value_col = "value".to_owned();
    let mut timeout_f = 5.0;
    let greeting = format!(
        "ndj2influx v{}. Sends metrics from STDIN (ndjson) to InfluxDB",
        VERSION
    );
    {
        let mut ap = ArgumentParser::new();
        ap.set_description(&greeting);
        ap.refer(&mut url)
            .add_argument("URL", Store, "InfluxDB URL:Port (without leading slash)")
            .required();
        ap.refer(&mut database)
            .add_argument("DB", Store, "InfluxDB database name (or Organizaiton if v2)")
            .required();
        ap.refer(&mut bucket)
            .add_option(
                &["-B", "--bucket"],
                Store,
                "Bucket to store values",
            )
            .metavar("Bucket");
        ap.refer(&mut basecol)
            .add_argument(
                "BASE",
                Store,
                "base (hostname, etc.), use @name to set fixed value",
            )
            .required();
        ap.refer(&mut auth)
            .add_option(
                &["-U", "--user"],
                Store,
                "username:password, if required. (better use INFLUXDB_AUTH env variable)",
            )
            .metavar("NAME");
        ap.refer(&mut time_col)
            .add_option(
                &["-T", "--time-col"],
                Store,
                "Time column, timestamp (seconds) or RFC3339, default: 'time'. \
                use '@' to ignore JSON data and set current
                time",
            )
            .metavar("NAME");
        ap.refer(&mut metric_col)
            .add_option(
                &["-M", "--metric-col"],
                Store,
                "Metric column (default: parse as K=V)",
            )
            .metavar("NAME");
        ap.refer(&mut value_col)
            .add_option(
                &["-V", "--value-col"],
                Store,
                "Value column (default: 'value', not used for K=V), non-numeric values are skipped",
            )
            .metavar("NAME");
        ap.refer(&mut timeout_f)
            .add_option(&["--timeout"], Store, "DB timeout (default: 5 sec)")
            .metavar("NAME");
        ap.refer(&mut verbose).add_option(
            &["-v", "--verbose"],
            StoreTrue,
            "Verbose output (debug)",
        );
        ap.parse_args_or_exit();
    }
    let use_v2 = !bucket.is_empty();

    let base = match basecol.chars().next().unwrap() {
        '@' => &basecol[1..basecol.len()],
        _ => "",
    };

    if !auth.is_empty() {
        if use_v2 {
            auth = format!("Token {}", auth);
        } else {
            auth = format!("Basic {}", &base64::encode(auth));
        }
    }

    let influx_write_url = if use_v2 {
        format!("{}/api/v2/write?org={}&bucket={}", url, database, bucket)
    } else {
        format!("{}/write?db={}", url, database)
    };

    let timeout = Duration::from_millis((timeout_f * 1000_f32) as u64);
    let stdin = io::stdin();
    let (tx, rx) = mpsc::channel();
    let processor = thread::spawn(move || loop {
        let q: String = rx.recv().unwrap();
        if q.is_empty() {
            break;
        }
        let mut client = ureq::post(&influx_write_url);
        if !auth.is_empty() {
            client.set("Authorization", &auth);
        }
        client.timeout(timeout);
        let res = client.send_string(&q);
        if !res.ok() {
            panic!("DB error {}: {}", res.status(), res.into_string().unwrap());
        }
    });
    loop {
        let mut buffer = String::new();
        match stdin.lock().read_line(&mut buffer) {
            Ok(v) => {
                if v == 0 {
                    break;
                }
            }
            Err(err) => panic!("{}", err),
        }
        if buffer != "\n" && buffer != "\r\n" && buffer != "\n\r" {
            let jmap: HashMap<String, serde_json::Value> = serde_json::from_str(&buffer).unwrap();
            let timestamp = parse_timestamp(&jmap, &time_col);
            let data = parse_metrics(&jmap, &metric_col, &value_col, &time_col, &basecol, verbose);
            let b = if base.is_empty() {
                parse_basecol(&jmap, &basecol)
            } else {
                base.to_owned()
            };
            if !data.is_empty() {
                let mut q = format!("{} ", b);
                let mut qm = "".to_owned();
                for (k, v) in data {
                    if !qm.is_empty() {
                        qm += ",";
                    }
                    qm.push_str(&format!("{}={}", k, v))
                }
                let qts = format!(" {}", timestamp);
                if verbose {
                    println!("{}{}{}", q.cyan().bold(), qm.blue().bold(), qts.green());
                }
                q = q + &qm + &qts;
                tx.send(q).unwrap();
            }
        }
    }
    tx.send("".to_owned()).unwrap();
    processor.join().unwrap();
}
