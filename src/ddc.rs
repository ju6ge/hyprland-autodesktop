use std::{convert::From, fmt::Debug};

use ddc_hi::{Display, Ddc};
use serde::{Deserialize, de::{Unexpected, Error}, Serialize};
use serde_yaml::Value;

const FEATURE_CODE_INPUT_SOURCE: u8 = 0x60;

#[derive(Debug, Serialize, Deserialize, PartialEq, Clone)]
#[serde(remote = "MonitorInputSource")]
pub enum MonitorInputSource {
    Vga1,
    Vga2,
    Dvi1,
    Dvi2,
    Hdmi1,
    Hdmi2,
    Dp1,
    Dp2,
    Other(u8)
}

impl MonitorInputSource {
    fn id(&self) -> u16 {
        match self {
            MonitorInputSource::Vga1 => 0x1,
            MonitorInputSource::Vga2 => 0x2,
            MonitorInputSource::Dvi1 => 0x3,
            MonitorInputSource::Dvi2 => 0x4,
            MonitorInputSource::Hdmi1 => 0xf,
            MonitorInputSource::Hdmi2 => 0x10,
            MonitorInputSource::Dp1 => 0x11,
            MonitorInputSource::Dp2 => 0x12,
            MonitorInputSource::Other(x) => *x as u16,
        }
    }
}

impl From<u8> for MonitorInputSource {
    fn from(value: u8) -> Self {
        match value {
            0x1 => Self::Vga1,
            0x2 => Self::Vga2,
            0x3 => Self::Dvi1,
            0x4 => Self::Dvi2,
            0xf => Self::Dp1,
            0x10 => Self::Dp2,
            0x11 => Self::Hdmi1,
            0x12 => Self::Hdmi2,
            _ => MonitorInputSource::Other(value)
        }
    }
}


impl Serialize for MonitorInputSource{
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer
    {
        MonitorInputSource::serialize(self, serializer)
    }
}

impl<'de> Deserialize<'de> for MonitorInputSource {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>
    {
        let value: Value = Deserialize::deserialize(deserializer)?;
        match value.clone() {
            Value::Number(v) => {
                if v.is_u64() {
                    let num = v.as_u64().unwrap();
                    if num <= 255 {
                        Ok(MonitorInputSource::from(num as u8))
                    } else {
                        Err(Error::invalid_value(Unexpected::Unsigned(num), &"expected u8!"))
                    }
                } else if v.is_i64() {
                    Err(Error::invalid_type(Unexpected::Signed(v.as_i64().unwrap()), &"expected u8!"))
                } else {
                    Err(Error::invalid_type(Unexpected::Float(v.as_f64().unwrap()), &"expected u8!"))
                }
            },
            _ => {
                MonitorInputSource::deserialize(value).map_err(|err| {
                    Error::custom(err.to_string())
                })
            }
        }
    }
}

pub struct DdcMonitor {
    display: Display
}

impl Debug for DdcMonitor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DdcMonitor").field("display", &self.display.info).finish()
    }
}

impl DdcMonitor {
    pub fn get_display_by_serial(serial: &str) -> Option<Self> {
        Display::enumerate().into_iter().for_each(|display| {
            display.info.serial_number.clone().and_then(|current_serial| {
                    if current_serial == serial {
                        return Some(Self{
                            display: display
                        });
                    }
                    None
                });
            }
        );
        None
    }     
    
    pub fn get_input_source(&mut self) -> Option<MonitorInputSource> {
        self.display.handle.get_vcp_feature(FEATURE_CODE_INPUT_SOURCE).map(|code| {
            code.ml.into()
        }).ok()
    }

    pub fn set_input_souce(&mut self, input: &MonitorInputSource) -> Result<(), anyhow::Error> {
        self.display.handle.set_vcp_feature(FEATURE_CODE_INPUT_SOURCE, input.id())
    }
}
