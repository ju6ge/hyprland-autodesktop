use std::fmt::Debug;

use libmonitor::mccs::features::InputSource;
use serde::{
    de::{Error, Unexpected},
    Deserialize, Serialize,
};
use serde_yaml::Value;

#[derive(Debug, Clone)]
pub struct MonitorInputSource(libmonitor::mccs::features::InputSource);

impl MonitorInputSource {
    pub fn input(&self) -> &InputSource {
        &self.0
    }
}

impl Serialize for MonitorInputSource {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let num: u32 = self.0.into();
        num.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for MonitorInputSource {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let value: Value = Deserialize::deserialize(deserializer)?;
        match value.clone() {
            Value::Number(v) => {
                if v.is_u64() {
                    let num = v.as_u64().unwrap();
                    if num <= 255 {
                        Ok(MonitorInputSource((num as u32).into()))
                    } else {
                        Err(Error::invalid_value(
                            Unexpected::Unsigned(num),
                            &"expected u8!",
                        ))
                    }
                } else if v.is_i64() {
                    Err(Error::invalid_type(
                        Unexpected::Signed(v.as_i64().unwrap()),
                        &"expected u8!",
                    ))
                } else {
                    Err(Error::invalid_type(
                        Unexpected::Float(v.as_f64().unwrap()),
                        &"expected u8!",
                    ))
                }
            }
            _ => {
                MonitorInputSource::deserialize(value).map_err(|err| Error::custom(err.to_string()))
            }
        }
    }
}
