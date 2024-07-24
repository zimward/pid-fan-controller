mod pid;
use anyhow::{anyhow, Context, Result};
use glob::{glob, GlobError};
use pid::Pid;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs::{read_to_string, write, File};
use std::io::Read;
use std::path::PathBuf;
use std::string::String;
use std::thread;
use std::time::Duration;

const CONFIG_FILE: &str = "/etc/pid-fan-settings.json";

struct HeatSrc {
    temp_input: PathBuf,
    pub last_pid: f32,
    pid: Pid,
}

fn read_c(path: &PathBuf, count: usize) -> Result<String> {
    let mut file = File::open(path)?;
    let mut buf = vec![0u8; count];
    let bytes_read = file.read(&mut buf)?;
    Ok(String::from_utf8(buf[..bytes_read].to_vec())?)
}

impl HeatSrc {
    const fn new(temp_input: PathBuf, pid: Pid) -> Self {
        Self {
            temp_input,
            last_pid: 0.0,
            pid,
        }
    }
    pub fn run_pwm(&mut self, interval: f32) -> Result<()> {
        //temperature is never longer than 7 bytes
        let mut temp = read_c(&self.temp_input, 7)?;
        temp.pop();
        let temp: f32 = temp.parse()?;
        self.last_pid = self.pid.run(temp, interval);
        Ok(())
    }
}

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
    ) -> Self {
        Self {
            min_pwm,
            #[allow(clippy::cast_precision_loss)]
            range: (max_pwm - min_pwm) as f32,
            cutoff,
            heat_pressure_srcs,
            pwm,
        }
    }
    fn set_speed(&self, speed: f32) -> Result<()> {
        let mut pwm_duty: u32;
        #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
        let round = (self.range * speed).round() as u32;
        pwm_duty = self.min_pwm + round;
        if pwm_duty == self.min_pwm && self.cutoff {
            pwm_duty = 0;
        }
        write(&self.pwm, pwm_duty.to_string().as_bytes())?;
        Ok(())
    }
    fn pwm_enable(&self, enable: bool) -> Result<()> {
        let mut path = self.pwm.clone();
        let path = path.as_mut_os_string();
        path.push("_enable");
        //0 - off 1 - manual; 2 - automatic
        let val = if enable { "1" } else { "2" };
        write(path, val.as_bytes())?;
        Ok(())
    }
}

fn resolve_file_path(path: &str) -> Result<PathBuf> {
    let iter = glob(path)?;
    let mut paths: Vec<Result<PathBuf, GlobError>> = iter.collect();
    if paths.len() > 1 {
        Err(anyhow!("Path {path} returns more than one result."))
    } else if let Some(path) = paths.pop() {
        Ok(path?)
    } else {
        Err(anyhow!("Path {path} returns no vaild result."))
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "UPPERCASE")]
struct PidCfg {
    p: f32,
    i: f32,
    d: f32,
    #[serde(rename = "set_point")]
    setpoint: f32,
}

#[derive(Deserialize)]
struct HeatSrcCfg {
    name: String,
    wildcard_path: String,
    #[serde(rename = "PID_params")]
    pid: PidCfg,
}

const fn def_max_pwm() -> u32 {
    255
}

#[derive(Deserialize)]
struct FanCfg {
    wildcard_path: String,
    min_pwm: u32,
    #[serde(default = "def_max_pwm")]
    max_pwm: u32,
    #[serde(default)]
    cutoff: bool,
    heat_pressure_srcs: Vec<String>,
}

const fn def_interval() -> u32 {
    500
}

#[derive(Deserialize)]
struct Config {
    heat_srcs: Vec<HeatSrcCfg>,
    fans: Vec<FanCfg>,
    #[serde(default = "def_interval")]
    interval: u32,
}

fn parse_config() -> Result<(Vec<HeatSrc>, Vec<Fan>, u32)> {
    let conf = read_to_string(CONFIG_FILE).context("Failed to read config File")?;
    let conf: Config = serde_json::from_str(&conf)?;
    let mut heat_srcs: Vec<HeatSrc> = Vec::default();
    let mut fans: Vec<Fan> = Vec::default();
    let mut heat_map: HashMap<String, usize> = HashMap::default();
    for (i, heat_src) in conf.heat_srcs.iter().enumerate() {
        let temp_input = resolve_file_path(&heat_src.wildcard_path);
        let pid = &heat_src.pid;
        heat_srcs.push(HeatSrc::new(
            temp_input?,
            Pid::new(pid.p, pid.i, pid.d, pid.setpoint * 1000.0),
        ));
        heat_map.insert(heat_src.name.clone(), i);
    }
    for fan in conf.fans {
        let pwm = resolve_file_path(&fan.wildcard_path);
        let mut heat_pressure_srcs: Vec<usize> = Vec::default();
        for src in fan.heat_pressure_srcs {
            heat_pressure_srcs.push(*heat_map.get(&src).context("Heat Source {src} not found")?);
        }
        let f = Fan::new(
            fan.min_pwm,
            fan.max_pwm,
            fan.cutoff,
            heat_pressure_srcs,
            pwm?,
        );
        fans.push(f);
    }

    Ok((heat_srcs, fans, conf.interval))
}

fn main() -> Result<()> {
    let (mut heat_srcs, fans, interval) = parse_config()?;
    #[allow(clippy::cast_precision_loss)]
    let interval_seconds: f32 = (interval as f32) / 1000.0;
    let mut enable = true;
    if let Some(arg) = std::env::args().nth(1) {
        if arg == "disable" {
            enable = false;
        }
    }
    for fan in &fans {
        fan.pwm_enable(enable)?;
    }
    if !enable {
        return Ok(());
    }
    loop {
        for heat_src in &mut heat_srcs {
            heat_src.run_pwm(interval_seconds)?;
        }
        for fan in &fans {
            let mut highest_pressure: f32 = 0.0;
            for prs_src in &fan.heat_pressure_srcs {
                if heat_srcs[*prs_src].last_pid > highest_pressure {
                    highest_pressure = heat_srcs[*prs_src].last_pid;
                }
            }
            fan.set_speed(highest_pressure)?;
        }
        thread::sleep(Duration::from_millis(interval.into()));
    }
}
