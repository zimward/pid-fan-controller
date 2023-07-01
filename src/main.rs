mod pid;
use glob::{glob, GlobError};
use pid::PID;
use serde::Deserialize;
use std::collections::HashMap;
use std::fmt::Debug;
use std::fs::{read_to_string, write, File};
use std::io::Read;
use std::path::PathBuf;
use std::string::String;
use std::thread;
use std::time::Duration;

const CONFIG_FILE: &'static str = "/etc/pid-fan-settings.json";

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
    fn new(temp_input: PathBuf, pid: PID) -> HeatSrc {
        HeatSrc {
            temp_input,
            last_pid: 0.0,
            pid,
        }
    }
    pub fn run_pwm(&mut self, interval: f32) {
        let mut tmp = read_c(&self.temp_input, 7).expect("Failed to read temperature");
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

fn resolve_file_path(path: String) -> PathBuf {
    let iter = glob(path.as_str()).expect("Failed to process glob");
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

#[derive(Debug, Deserialize)]
struct HeatSrcCfg {
    name: String,
    wildcard_path: String,
    #[serde(rename = "PID_params")]
    pid: PID,
}

fn def_max_pwm() -> u32 {
    255
}

#[derive(Debug, Deserialize)]
struct FanCfg {
    wildcard_path: String,
    min_pwm: u32,
    #[serde(default = "def_max_pwm")]
    max_pwm: u32,
    #[serde(default)]
    cutoff: bool,
    heat_pressure_srcs: Vec<String>,
}

fn def_interval() -> u32 {
    500
}

#[derive(Deserialize)]
struct Config {
    heat_srcs: Vec<HeatSrcCfg>,
    fans: Vec<FanCfg>,
    #[serde(default = "def_interval")]
    interval: u32,
}

fn parse_config() -> (Vec<HeatSrc>, Vec<Fan>, u32) {
    let conf = read_to_string(CONFIG_FILE).expect("Error reading config file");
    let conf: Config = serde_json::from_str(&conf).expect("Error Parsing config file");
    let mut heat_srcs: Vec<HeatSrc> = Vec::default();
    let mut fans: Vec<Fan> = Vec::default();
    let mut heat_map: HashMap<String, usize> = HashMap::default();
    for (i, heat_src) in conf.heat_srcs.iter().enumerate() {
        let temp_input = resolve_file_path(heat_src.wildcard_path.clone());
        let pid = &heat_src.pid;
        heat_srcs.push(HeatSrc::new(
            temp_input,
            PID::new(pid.p, pid.i, pid.d, pid.setpoint * 1000.0),
        ));
        heat_map.insert(heat_src.name.clone(), i);
    }
    for fan in conf.fans {
        let pwm = resolve_file_path(fan.wildcard_path);
        let mut heat_pressure_srcs: Vec<usize> = Vec::default();
        for src in fan.heat_pressure_srcs {
            heat_pressure_srcs.push(
                heat_map
                    .get(&src)
                    .expect("Heat source {src} not found")
                    .clone(),
            );
        }
        let f = Fan::new(
            fan.min_pwm,
            fan.max_pwm,
            fan.cutoff,
            heat_pressure_srcs,
            pwm,
        );
        fans.push(f);
    }

    (heat_srcs, fans, conf.interval)
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
    if !enable {
        return;
    }
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
