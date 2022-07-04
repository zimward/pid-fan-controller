mod pid;
use lite_json::*;
use pid::PID;
use std::borrow::BorrowMut;
use std::fmt::Debug;
use std::fs::read_to_string;
use std::string::String;

const CONFIG_FILE: &'static str = "./fan_settings.json";
const INTERVAL: i32 = 500; //in ms

#[derive(Debug)]
struct HeatSrc {
    pub temp_input: String,
    pub pid: PID,
}

impl HeatSrc {
    fn new(temp_input: String, p: f32, i: f32, d: f32, set_point: f32) -> HeatSrc {
        HeatSrc {
            temp_input,
            pid: PID::new(p, i, d, set_point),
        }
    }
}
#[derive(Debug)]
struct Fan {
    min: i32,
    max: i32,
    cutoff: bool,
    heat_srcs: Vec<i32>,
    pwm: String,
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

fn handle_srcs(srcs: Vec<JsonValue>) -> Vec<(String, HeatSrc)> {
    let mut configured_srcs: Vec<(String, HeatSrc)> = Vec::with_capacity(srcs.len());
    for src in srcs {
        let mut temp_input: String = "".to_string();
        let mut name: String = "".to_string();
        let mut set_point: f32 = 0.0;
        let mut p: f32 = 0.0;
        let mut i: f32 = 0.0;
        let mut d: f32 = 0.0;

        let src = get_object(src).expect("heat sources have to be defined as objects");
        for e in src {
            let k: String = e.0.into_iter().collect();
            match k.as_str() {
                "name" => {
                    name = get_string(e.1).expect("'name' of heat source must be a string");
                }
                "wildcard_path" => {
                    temp_input =
                        get_string(e.1).expect("'wildcard_path' of heat source must be a string");
                }
                "PID_params" => {
                    let params = get_object(e.1).expect("'PID_params' has to be an Object");
                    for e in params {
                        let k: String = e.0.into_iter().collect();
                        match k.as_str() {
                            "set_point" => {
                                set_point = get_number(e.1).expect("'set_point' must be a number");
                            }
                            "P" => {
                                p = -get_number(e.1).expect("'P' must be a number");
                            }
                            "I" => {
                                i = -get_number(e.1).expect("'I' must be a number");
                            }
                            "D" => {
                                d = -get_number(e.1).expect("'D' must be a number");
                            }
                            &_ => {}
                        }
                    }
                }
                &_ => {}
            }
        }
        configured_srcs.push((name, HeatSrc::new(temp_input, p, i, d, set_point)));
    }
    configured_srcs
}

fn handle_fans(fans: Vec<JsonValue>) -> Vec<(String, i32, i32, bool, Vec<String>)> {
    let mut configured_fans: Vec<(String, i32, i32, bool, Vec<String>)> =
        Vec::with_capacity(fans.len());
    for e in fans {
        let mut wildcard_path: String = "".to_string();
        let mut min_pwm: i32 = 0;
        let mut max_pwm: i32 = 255;
        let mut cutoff: bool = true;
        let mut heat_srcs: Option<Vec<String>> = None;
        let fan = get_object(e).expect("fan entries have to be objects");
        for e in fan {
            let k: String = e.0.into_iter().collect();
            match k.as_str() {
                "wildcard_path" => {
                    wildcard_path =
                        get_string(e.1).expect("'wildcard_path' of fan has to be a string");
                }
                "min_pwm" => {
                    min_pwm = get_integer(e.1).expect("'min_pwm' has to be a integer");
                }
                "max_pwm" => {
                    max_pwm = get_integer(e.1).expect("'max_pwm' has to be a integer");
                }
                "cutoff" => {
                    if let JsonValue::Boolean(val) = e.1 {
                        cutoff = val;
                    } else {
                        panic!("'cutoff' has to be a boolean");
                    }
                }
                "heat_pressure_srcs" => {
                    let srcs = get_array(e.1).expect("'heat_pressure_srcs' has to be a array");
                    if heat_srcs == None {
                        heat_srcs = Some(Vec::with_capacity(srcs.len()));
                    }
                    for src in srcs {
                        if let Some(heat_srcs) = heat_srcs.borrow_mut() {
                            heat_srcs.push(
                                get_string(src)
                                    .expect("'heat_pressure_srcs' array may only contain strings"),
                            );
                        }
                    }
                }
                &_ => {}
            }
        }
        if wildcard_path.len() == 0 {
            panic!("'wildcard_path' is a mandatory fan parameter");
        }
        let srcs = heat_srcs.expect("fan must have 'heat_pressure_srcs'");
        configured_fans.push((wildcard_path, min_pwm, max_pwm, cutoff, srcs));
    }
    dbg!(configured_fans.clone());
    configured_fans
}

fn parse_config() {
    let config = match parse_json(&read_to_string(CONFIG_FILE).expect("Error reading config file"))
    {
        Ok(cfg) => cfg,
        Err(err) => {
            panic!("Error parsing config file.");
        }
    };
    let cfg = get_object(config).expect("config must be wrap in an object");
    let mut heat_srcs: Vec<(String, HeatSrc)> = Vec::new();
    for e in cfg {
        let typ: String = e.0.into_iter().collect();
        match typ.as_str() {
            "heat_srcs" => {
                heat_srcs = handle_srcs(get_array(e.1).expect("'heat_srcs' must be a array"));
            }
            "fans" => {
                handle_fans(get_array(e.1).expect("'fans' mut be a array"));
            }
            &_ => {}
        }
    }
    //    let heat_srcs: Vec<HeatSrc> = Vec::with_capacity(config.get("heat_srcs"));
    //    let fans: Vec<Fan> = Vec::new();
}

fn main() {
    parse_config();
}
