use std::{
    collections::HashMap, fs::OpenOptions, io::Read, os::unix::process::CommandExt,
    process::Command, str::FromStr, thread,
};

use gtk::gdk::RGBA;
use gtk4_layer_shell::Edge;

use crate::ui::EventMap;

#[derive(Debug, Clone)]
pub enum MonitorSpecifier {
    ID(usize),
    Name(String),
}

pub struct Config {
    pub edge: Edge,
    pub position: Option<Edge>,
    pub size: (f64, f64),
    pub event_map: Option<EventMap>,
    pub color: RGBA,
    pub transition_duration: u64,
    pub frame_rate: u64,
    pub extra_trigger_size: f64,
    pub monitor: MonitorSpecifier,
}
#[derive(Debug)]
struct Test {
    edge: Edge,
    position: Option<Edge>,
    size: (f64, f64),
    color: RGBA,
    transition_duration: u64,
    frame_rate: u64,
    extra_trigger_size: f64,
    monitor: MonitorSpecifier,
}
impl Config {
    pub fn debug(&self) -> String {
        format!(
            "{:#?}",
            Test {
                edge: self.edge,
                position: self.position,
                size: self.size,
                color: self.color,
                transition_duration: self.transition_duration,
                frame_rate: self.frame_rate,
                extra_trigger_size: self.extra_trigger_size,
                monitor: self.monitor.clone(),
            }
        )
    }
}
impl Drop for Config {
    fn drop(&mut self) {
        println!("dropping config")
    }
}

use serde::Deserialize;
#[derive(Deserialize, Debug, Default)]
#[serde(default)]
pub struct RawConfig {
    #[serde(default = "dt_edge")]
    pub edge: String,
    #[serde(default)]
    pub position: String,
    #[serde(default = "dt_width")]
    pub width: f64,
    #[serde(default = "dt_height")]
    pub height: f64,
    // #[serde(default = "dt_rel_height")]
    // pub rel_height: f64,
    #[serde(default)]
    pub event_map: Vec<(u32, String)>,
    #[serde(default = "dt_color")]
    pub color: String,
    #[serde(default = "dt_duration")]
    pub transition_duration: u64,
    #[serde(default = "dt_frame_rate")]
    pub frame_rate: u64,
    #[serde(default = "dt_trigger_size")]
    pub extra_trigger_size: f64,
    #[serde(default)]
    monitor_id: usize,
    #[serde(default)]
    monitor_name: String,
}
fn dt_edge() -> String {
    String::from("left")
}
fn dt_width() -> f64 {
    15.
}
fn dt_height() -> f64 {
    50.
}
fn dt_color() -> String {
    String::from("#7B98FF")
}
fn dt_duration() -> u64 {
    300
}
fn dt_frame_rate() -> u64 {
    30
}
fn dt_trigger_size() -> f64 {
    5.
}

#[derive(Deserialize, Debug)]
struct RawGroup {
    #[serde(default)]
    name: String,
    #[serde(default)]
    widgets: Vec<RawConfig>,
}
#[derive(Deserialize, Debug)]
struct RawTemp {
    #[serde(default)]
    groups: Vec<RawGroup>,
}

pub type GroupConfigMap = HashMap<String, GroupConfig>;
pub type GroupConfig = Vec<Config>;

pub fn get_config_test() {
    let res = get_config().unwrap();

    res.iter().for_each(|(name, vc)| {
        println!("name: {name}");
        vc.iter().for_each(|c| {
            println!("{}", c.debug());
        });
    });
}

pub fn parse_config_test() {
    let data = r#"
    {
        "$schema": "sfa",
        "groups": [
            {
                "name": "test",
                "widgets": [{
                    "event_map": [[ 0, "ee" ]]
                }]
            }
        ]
    }
    "#;
    let res = parse_config(data).unwrap();

    res.iter().for_each(|(name, vc)| {
        println!("name: {name}");
        vc.iter().for_each(|c| {
            println!("{}", c.debug());
        });
    });
}

fn parse_config(data: &str) -> Result<GroupConfigMap, String> {
    let res: RawTemp = serde_jsonrc::from_str(data).unwrap();
    let mut group_map: GroupConfigMap = HashMap::new();
    res.groups
        .into_iter()
        .try_for_each(|g| -> Result<(), String> {
            let name = g.name.clone();
            let vc = raw_2_conf(g)?;
            group_map.insert(name, vc);
            Ok(())
        })?;
    Ok(group_map)
}

fn raw_2_conf(raw: RawGroup) -> Result<GroupConfig, String> {
    raw.widgets
        .into_iter()
        .map(|raw| -> Result<Config, String> {
            let edge = match raw.edge.as_str() {
                "top" => Edge::Top,
                "left" => Edge::Left,
                "bottom" => Edge::Bottom,
                "right" => Edge::Right,
                _ => {
                    let a = Err(format!("invalid edge {}", raw.edge));
                    return a;
                }
            };
            let position = match raw.position.as_str() {
                "top" => Some(Edge::Top),
                "left" => Some(Edge::Left),
                "bottom" => Some(Edge::Bottom),
                "right" => Some(Edge::Right),
                "" | "center" => None,
                _ => {
                    let a = Err(format!("invalid position {}", raw.position));
                    return a;
                }
            };
            let width = {
                if raw.width <= 0. {
                    return Err("width must be > 0".to_string());
                }
                raw.width
            };
            let height = {
                if raw.height <= 0. {
                    return Err("height must be > 0".to_string());
                }
                raw.height
            };
            if width * 2. > height {
                return Err("width * 2 must be <= height".to_string());
            }
            let event_map = {
                let mut map = EventMap::new();
                for (key, value) in raw.event_map {
                    map.insert(
                        key,
                        Box::new(move || {
                            let value = value.clone();
                            thread::spawn(move || {
                                Command::new("/bin/sh")
                                    .arg("-c")
                                    .arg(&value)
                                    .output()
                                    .unwrap();
                            });
                        }),
                    );
                }
                map
            };
            let color = match RGBA::from_str(&raw.color) {
                Ok(c) => c,
                Err(e) => {
                    return Err(format!("invalid color {}", e));
                }
            };
            let transition_duration = raw.transition_duration;
            let frame_rate = raw.frame_rate;
            let extra_trigger_size = raw.extra_trigger_size;
            let monitor = {
                if raw.monitor_name.is_empty() {
                    MonitorSpecifier::ID(raw.monitor_id)
                } else {
                    MonitorSpecifier::Name(raw.monitor_name)
                }
            };

            Ok(Config {
                edge,
                position,
                size: (width, height),
                event_map: Some(event_map),
                color,
                transition_duration,
                frame_rate,
                extra_trigger_size,
                monitor,
            })
        })
        .collect()
}

fn get_config_file() -> Result<String, String> {
    let bd = match xdg::BaseDirectories::new() {
        Ok(bd) => bd,
        Err(e) => return Err(format!("no xdg base directories: {e}")),
    };

    let p = match bd.place_config_file("way-edges/config.jsonc") {
        Ok(p) => p,
        Err(e) => return Err(format!("failed to create config file: {e}")),
    };

    let mut f = match OpenOptions::new()
        .create(true)
        .append(true)
        .read(true)
        .open(p)
    {
        Ok(f) => f,
        Err(e) => return Err(format!("failed to open config file: {e}")),
    };
    let mut s = String::new();
    match f.read_to_string(&mut s) {
        Ok(_) => {}
        Err(e) => return Err(format!("failed to read config file: {e}")),
    };
    Ok(s)
}

pub fn get_config() -> Result<GroupConfigMap, String> {
    let s = get_config_file().unwrap();
    parse_config(&s)
}

pub fn match_group_config(group_map: GroupConfigMap, group: &Option<String>) -> GroupConfig {
    if group_map.is_empty() {
        panic!("empty config");
    }
    if let Some(group_name) = group {
        group_map
            .into_iter()
            .find(|(n, _)| n == group_name)
            .unwrap_or_else(|| panic!("group not found given name: {group_name}"))
            .1
    } else if group_map.len() == 1 {
        group_map.into_values().last().unwrap()
    } else {
        panic!("no group available");
    }
}
