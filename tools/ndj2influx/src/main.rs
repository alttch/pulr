use argparse::{ArgumentParser, Store, StoreTrue};
use chrono::DateTime;
use colored::Colorize;
use reqwest;
use std::collections::HashMap;
use std::io::{self, BufRead};
use std::sync::mpsc;
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

fn parse_timestamp(map: &HashMap<String, serde_json::Value>, tcol: &String) -> i64 {
    return match tcol.as_str() {
        "@" => SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("Time went backwards")
            .as_nanos() as i64,
        _ => {
            let t = map
                .get(tcol)
                .expect(&format!("Time column not found: {}", tcol));
            use serde_json::value::Value::*;
            match t {
                Number(v) => (v.as_f64().unwrap() * 1_000_000_000.0) as i64,
                String(v) => {
                    let dt = DateTime::parse_from_rfc3339(&v).unwrap();
                    dt.timestamp_nanos()
                }
                _ => panic!("time is in wrong format: {:?}", t),
            }
        }
    };
}

fn parse_value(val: &serde_json::Value, verbose: bool) -> Option<f64> {
    use serde_json::value::Value::*;
    return match val {
        Number(v) => v.as_f64(),
        _ => {
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
    };
}

fn parse_basecol(map: &HashMap<String, serde_json::Value>, basecol: &String) -> String {
    let value = map
        .get(basecol)
        .expect(&format!("base column not found: {}", basecol));
    use serde_json::value::Value::*;
    return match value {
        String(v) => v.to_owned(),
        _ => panic!("base in wrong format: {:?}", value),
    };
}

fn parse_metrics(
    map: &HashMap<String, serde_json::Value>,
    mcol: &String,
    vcol: &String,
    tcol: &String,
    basecol: &String,
    verbose: bool,
) -> HashMap<String, f64> {
    let mut data: HashMap<String, f64> = HashMap::new();
    if mcol == "" {
        for (metric, val) in map {
            if metric != tcol && metric != basecol {
                match parse_value(val, verbose) {
                    Some(v) => {
                        let _ = data.insert(metric.to_owned(), v);
                    }
                    None => { // skip non-numeric }
                    }
                }
            }
        }
    } else {
        use serde_json::value::Value::*;
        let mj = map
            .get(mcol)
            .expect(&format!("metric col not found: {}", mcol));
        let metric = match mj {
            String(v) => v,
            _ => panic!("metric ID in wrong format: {}"),
        };
        let val = map
            .get(vcol)
            .expect(&format!("value col not found: {}", vcol));
        match parse_value(val, verbose) {
            Some(v) => {
                let _ = data.insert(metric.to_owned(), v);
            }
            None => { // skip non-numeric }
            }
        }
    }
    return data;
}

fn main() {
    let mut basecol = String::new();
    let mut url = String::new();
    let mut database = String::new();
    let mut auth = String::new();
    let mut verbose: bool = false;
    let mut tcol = "time".to_owned();
    let mut mcol = String::new();
    let mut vcol = "value".to_owned();
    let greeting = "Sends metrics from STDIN (ndjson) to InfluxDB";
    {
        let mut ap = ArgumentParser::new();
        ap.set_description(greeting);
        ap.refer(&mut url)
            .add_argument("URL", Store, "InfluxDB URL:Port (without leading slash)")
            .required();
        ap.refer(&mut database)
            .add_argument("DB", Store, "InfluxDB database name")
            .required();
        ap.refer(&mut basecol)
            .add_argument(
                "BASE",
                Store,
                "base (hostname, etc.), use @name to set fixed value",
            )
            .required();
        ap.refer(&mut auth)
            .add_option(&["-U", "--user"], Store, "username:password, if required")
            .metavar("NAME");
        ap.refer(&mut tcol)
            .add_option(
                &["-T", "--time-col"],
                Store,
                "Time column, timestamp (seconds) or RFC3339, default: 'time'. \
                use '@' to ignore JSON data and set current
                time",)
            .metavar("NAME");
        ap.refer(&mut mcol)
            .add_option(
                &["-M", "--metric-col"],
                Store,
                "Metric column (default: parse as K=V)",
            )
            .metavar("NAME");
        ap.refer(&mut vcol)
            .add_option(
                &["-V", "--value-col"],
                Store,
                "Value column (default: 'value', not used for K=V), non-numeric values are skipped",
            )
            .metavar("NAME");
        ap.refer(&mut verbose).add_option(
            &["-v", "--verbose"],
            StoreTrue,
            "Verbose output (debug)",
        );
        ap.parse_args_or_exit();
    }
    let (username, password) = match auth.is_empty() {
        true => ("".to_owned(), "".to_owned()),
        false => {
            let mut n = auth.splitn(2, ":");
            (n.next().unwrap().to_owned(), n.next().unwrap().to_owned())
        }
    };
    let base = match basecol.chars().next().unwrap() {
        '@' => &basecol[1..basecol.len()],
        _ => "",
    };
    let influx_write_url = format!("{}/write?db={}", url, database);
    let stdin = io::stdin();
    let (tx, rx) = mpsc::channel();
    let processor = thread::spawn(move || {
        let client = reqwest::blocking::Client::new();
        loop {
            let q: String = rx.recv().unwrap();
            if q.is_empty() {
                break;
            }
            let builder = match username.is_empty() {
                true => client.post(&influx_write_url).body(q),
                false => client
                    .post(&influx_write_url)
                    .body(q)
                    .basic_auth(&username, Some(&password)),
            };
            let res = builder.send().expect("InfluxDB error");
            let status = res.status();
            if !status.is_success() {
                panic!("DB response {}", status);
            }
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
            let timestamp = parse_timestamp(&jmap, &tcol);
            let data = parse_metrics(&jmap, &mcol, &vcol, &tcol, &basecol, verbose);
            let b = match base.is_empty() {
                true => parse_basecol(&jmap, &basecol),
                false => base.to_owned(),
            };
            if !data.is_empty() {
                let mut q = format!("{} ", b);
                let mut qm = "".to_owned();
                for (k, v) in data {
                    if !qm.is_empty() {
                        qm = qm + ",";
                    }
                    qm = qm + &format!("{}={}", k, v);
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
