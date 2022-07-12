mod pid;
use glob::{glob, GlobError};
use lite_json::*;
use pid::PID;
use std::borrow::BorrowMut;
use std::collections::HashMap;
use std::fmt::Debug;
use std::fs::{read_to_string, write, File};
use std::io::Read;
use std::path::PathBuf;
use std::string::String;
use std::thread;
use std::time::Duration;

const CONFIG_FILE: &'static str = "/etc/pid-fan-settings.json";

trait InformedCrash<T> {
    fn unexpect(self, message: &str) -> T;
}

impl<T, E> InformedCrash<T> for Result<T, E> {
    fn unexpect(self, message: &str) -> T {
        if let Ok(val) = self {
            return val;
        } else {
            eprintln!("{}", message);
            std::process::exit(0xFF);
        }
    }
}
impl<T> InformedCrash<T> for Option<T> {
    fn unexpect(self, message: &str) -> T {
        if let Some(val) = self {
            return val;
        } else {
            eprintln!("{}", message);
            std::process::exit(0xFF);
        }
    }
}

#[derive(Debug)]
struct HeatSrc {
    temp_input: PathBuf,
    pub last_pid: f32,
    pid: PID,
}

fn read_c(path: &PathBuf, count: usize) -> Result<String, std::string::FromUtf8Error> {
    let mut file = File::open(path).unwrap();
    let mut buf = vec![0u8; count];
    let bytes_read = file.read(&mut buf).unwrap();
    String::from_utf8(buf[..bytes_read].to_vec())
}

impl HeatSrc {
    fn new(temp_input: PathBuf, p: f32, i: f32, d: f32, set_point: f32) -> HeatSrc {
        HeatSrc {
            temp_input,
            last_pid: 0.0,
            pid: PID::new(p, i, d, set_point),
        }
    }
    pub fn run_pwm(&mut self, interval: f32) {
        let mut tmp = read_c(&self.temp_input, 7).unexpect("Failed to read temperature");
        tmp.pop();
        let temp: f32 = tmp.parse().unwrap();
        self.last_pid = self.pid.run(temp, interval);
    }
}
#[derive(Debug)]
struct Fan {
    min_pwm: u32,
    range: f32,
    cutoff: bool,
    heat_pressure_srcs: Vec<usize>,
    pwm: PathBuf,
}

impl Fan {
    fn new(
        min_pwm: u32,
        max_pwm: u32,
        cutoff: bool,
        heat_pressure_srcs: Vec<usize>,
        pwm: PathBuf,
    ) -> Fan {
        Fan {
            min_pwm,
            range: (max_pwm - min_pwm) as f32,
            cutoff,
            heat_pressure_srcs,
            pwm,
        }
    }
    fn set_speed(&self, speed: f32) {
        let mut pwm_duty: u32;
        pwm_duty = self.min_pwm + (self.range * speed).round() as u32;
        if pwm_duty == self.min_pwm && self.cutoff {
            pwm_duty = 0;
        }
        write(&self.pwm, pwm_duty.to_string().as_bytes()).unwrap();
    }
    fn pwm_enable(&self, enable: bool) {
        let mut path = self.pwm.clone();
        let mut filename = path.file_name().unwrap().to_string_lossy().to_string();
        filename.push_str("_enable");
        path.pop();
        path.push(filename);
        let val: u32 = match enable {
            true => 1,
            false => 0,
        };
        write(path, val.to_string().as_bytes()).unwrap();
    }
}

fn get_number(value: JsonValue) -> Option<f32> {
    if let JsonValue::Number(num) = value {
        return Some(num.to_f64() as f32);
    }
    None
}

fn get_integer(value: JsonValue) -> Option<i32> {
    if let JsonValue::Number(num) = value {
        return Some(num.integer as i32);
    }
    None
}

fn get_string(value: JsonValue) -> Option<String> {
    if let JsonValue::String(val) = value {
        return Some(val.into_iter().collect());
    }
    None
}

fn get_array(value: JsonValue) -> Option<Vec<JsonValue>> {
    if let JsonValue::Array(val) = value {
        return Some(val);
    }
    None
}
fn get_object(value: JsonValue) -> Option<JsonObject> {
    if let JsonValue::Object(val) = value {
        return Some(val);
    }
    None
}

fn resolve_file_path(path: String) -> PathBuf {
    let iter = glob(path.as_str()).unexpect("Failed to process glob");
    let paths: Vec<Result<PathBuf, GlobError>> = iter.collect();
    if paths.len() > 1 {
        eprintln!("Path {} returns more than one result.", path.as_str());
        std::process::exit(0xFF);
    } else if paths.len() == 0 {
        eprintln!("Path {} returns no vaild result.", path.as_str());
        std::process::exit(0xFF);
    }
    paths[0].as_ref().unwrap().to_path_buf()
}

fn handle_srcs(srcs: Vec<JsonValue>) -> Vec<(String, HeatSrc)> {
    let mut configured_srcs: Vec<(String, HeatSrc)> = Vec::with_capacity(srcs.len());
    for src in srcs {
        let mut temp_input: String = "".to_string();
        let mut name: String = "".to_string();
        let mut set_point: f32 = 0.0;
        let mut p: f32 = 0.0;
        let mut i: f32 = 0.0;
        let mut d: f32 = 0.0;

        let src = get_object(src).unexpect("heat sources have to be defined as objects");
        for e in src {
            let k: String = e.0.into_iter().collect();
            match k.as_str() {
                "name" => {
                    name = get_string(e.1).unexpect("'name' of heat source must be a string");
                }
                "wildcard_path" => {
                    temp_input =
                        get_string(e.1).unexpect("'wildcard_path' of heat source must be a string");
                }
                "PID_params" => {
                    let params = get_object(e.1).unexpect("'PID_params' has to be an Object");
                    for e in params {
                        let k: String = e.0.into_iter().collect();
                        match k.as_str() {
                            "set_point" => {
                                //the sysfs reading is in m°C
                                set_point = get_number(e.1)
                                    .unexpect("'set_point' must be a number")
                                    * 1000.0;
                            }
                            "P" => {
                                p = get_number(e.1).unexpect("'P' must be a number");
                            }
                            "I" => {
                                i = get_number(e.1).unexpect("'I' must be a number");
                            }
                            "D" => {
                                d = get_number(e.1).unexpect("'D' must be a number");
                            }
                            &_ => {}
                        }
                    }
                }
                &_ => {}
            }
        }
        configured_srcs.push((
            name,
            HeatSrc::new(resolve_file_path(temp_input), p, i, d, set_point),
        ));
    }
    configured_srcs
}

fn handle_fans(fans: Vec<JsonValue>) -> Vec<(String, u32, u32, bool, Vec<String>)> {
    let mut configured_fans: Vec<(String, u32, u32, bool, Vec<String>)> =
        Vec::with_capacity(fans.len());
    for e in fans {
        let mut wildcard_path: String = "".to_string();
        let mut min_pwm: u32 = 0;
        let mut max_pwm: u32 = 255;
        let mut cutoff: bool = false;
        let mut heat_srcs: Option<Vec<String>> = None;
        let fan = get_object(e).unexpect("fan entries have to be objects");
        for e in fan {
            let k: String = e.0.into_iter().collect();
            match k.as_str() {
                "wildcard_path" => {
                    wildcard_path =
                        get_string(e.1).unexpect("'wildcard_path' of fan has to be a string");
                }
                "min_pwm" => {
                    min_pwm = get_integer(e.1).unexpect("'min_pwm' has to be a integer") as u32;
                }
                "max_pwm" => {
                    max_pwm = get_integer(e.1).unexpect("'max_pwm' has to be a integer") as u32;
                }
                "cutoff" => {
                    if let JsonValue::Boolean(val) = e.1 {
                        cutoff = val;
                    } else {
                        eprintln!("'cutoff' has to be a boolean");
                        std::process::exit(0xFF);
                    }
                }
                "heat_pressure_srcs" => {
                    let srcs = get_array(e.1).unexpect("'heat_pressure_srcs' has to be a array");
                    if heat_srcs == None {
                        heat_srcs = Some(Vec::with_capacity(srcs.len()));
                    }
                    for src in srcs {
                        if let Some(heat_srcs) = heat_srcs.borrow_mut() {
                            heat_srcs.push(
                                get_string(src).unexpect(
                                    "'heat_pressure_srcs' array may only contain strings",
                                ),
                            );
                        }
                    }
                }
                &_ => {}
            }
        }
        if wildcard_path.len() == 0 {
            eprintln!("'wildcard_path' is a mandatory fan parameter");
            std::process::exit(0xFF);
        }
        let srcs = heat_srcs.unexpect("fan must have 'heat_pressure_srcs'");
        configured_fans.push((wildcard_path, min_pwm, max_pwm, cutoff, srcs));
    }
    configured_fans
}

fn parse_config() -> (Vec<HeatSrc>, Vec<Fan>, u32) {
    let config =
        match parse_json(&read_to_string(CONFIG_FILE).unexpect("Error reading config file")) {
            Ok(cfg) => cfg,
            Err(_err) => {
                eprintln!("Error parsing config file.");
                std::process::exit(0xFF);
            }
        };
    let cfg = get_object(config).unexpect("config must be wrap in an object");
    let mut heat_srcs: Vec<(String, HeatSrc)> = Vec::new();
    let mut fans: Vec<(String, u32, u32, bool, Vec<String>)> = Vec::new();
    let mut interval: u32 = 500;
    for e in cfg {
        let typ: String = e.0.into_iter().collect();
        match typ.as_str() {
            "heat_srcs" => {
                heat_srcs = handle_srcs(get_array(e.1).unexpect("'heat_srcs' must be a array"));
            }
            "fans" => {
                fans = handle_fans(get_array(e.1).unexpect("'fans' mut be a array"));
            }
            "interval" => {
                interval = get_integer(e.1)
                    .unexpect("'interval' must be a number")
                    .try_into()
                    .unexpect("interval must be positive.");
            }
            &_ => {}
        }
    }
    let mut name_lookup: HashMap<String, usize> = HashMap::with_capacity(heat_srcs.len());
    let mut fin_heat_srcs: Vec<HeatSrc> = Vec::with_capacity(heat_srcs.len());
    for (name, src) in heat_srcs {
        fin_heat_srcs.push(src);
        name_lookup.insert(name, fin_heat_srcs.len() - 1);
    }
    let mut fin_fans: Vec<Fan> = Vec::with_capacity(fans.len());
    for (pwm, min_pwm, max_pwm, cutoff, heat_pressure_srcs) in fans {
        let mut heat_prs_srcs: Vec<usize> = Vec::with_capacity(heat_pressure_srcs.len());
        for src in heat_pressure_srcs {
            let k = name_lookup
                .get(&src)
                .unexpect("heat_pressure_src entry is wrong");
            heat_prs_srcs.push(*k);
        }
        fin_fans.push(Fan::new(
            min_pwm,
            max_pwm,
            cutoff,
            heat_prs_srcs,
            resolve_file_path(pwm),
        ));
    }
    (fin_heat_srcs, fin_fans, interval)
}

fn main() {
    let (mut heat_srcs, fans, interval) = parse_config();
    let interval_seconds: f32 = (interval as f32) / 1000.0;
    let mut enable = true;
    if let Some(arg) = std::env::args().nth(1) {
        if arg == "disable" {
            enable = false;
        }
    }
    for fan in &fans {
        fan.pwm_enable(enable);
    }
    if enable {
        loop {
            for heat_src in &mut heat_srcs {
                heat_src.run_pwm(interval_seconds);
            }
            for fan in &fans {
                let mut highest_pressure: f32 = 0.0;
                for prs_src in &fan.heat_pressure_srcs {
                    if heat_srcs[*prs_src].last_pid > highest_pressure {
                        highest_pressure = heat_srcs[*prs_src].last_pid;
                    }
                }
                fan.set_speed(highest_pressure);
            }
            thread::sleep(Duration::from_millis(interval.into()));
        }
    }
}
