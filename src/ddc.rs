use std::fmt::Debug;

use libmonitor::mccs::features::InputSource;
use serde::{
    de::{Error, Unexpected},
    Deserialize, Serialize,
};
use serde_yaml::Value;

#[derive(Debug, Clone, PartialEq)]
pub enum MonitorInputSourceMatcher {
    Any,
    Input(libmonitor::mccs::features::InputSource)
}

impl MonitorInputSourceMatcher {
    pub fn matches(&self, input: InputSource) -> bool {
        match self {
            MonitorInputSourceMatcher::Any => true,
            MonitorInputSourceMatcher::Input(self_input) => *self_input == input,
        }
    }
}

impl Default for MonitorInputSourceMatcher {
    fn default() -> Self {
        Self::Any
    }
}

impl Serialize for MonitorInputSourceMatcher {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            MonitorInputSourceMatcher::Any => serializer.serialize_unit_variant("MonitorInputSourceMatcher", 0, "Any"),
            MonitorInputSourceMatcher::Input(input_source) => {
                match input_source {
                    InputSource::Reserved(val) => val.serialize(serializer),
                    _ => {
                        input_source.serialize(serializer)
                    }
                }
            },
        }
    }
}

#[cfg(test)]
mod test {
    use super::MonitorInputSourceMatcher;

    #[test]
    fn serialize_input_matcher() {
        let s = serde_yaml::to_string(&MonitorInputSourceMatcher::Any);
        assert!(s.is_ok_and(|val| val == "Any\n"));
        let s = serde_yaml::to_string(&MonitorInputSourceMatcher::Input(libmonitor::mccs::features::InputSource::Analog1));
        assert!(s.is_ok_and(|val| val == "Analog1\n"));
        let s = serde_yaml::to_string(&MonitorInputSourceMatcher::Input(libmonitor::mccs::features::InputSource::Reserved(100)));
        assert!(s.is_ok_and(|val| val == "100\n"));
    }

    #[test]
    fn deserialize_input_matcher() {
        let d: Result<MonitorInputSourceMatcher, serde_yaml::Error> = serde_yaml::from_str("Any");
        assert!( d.is_ok_and(|input| input == MonitorInputSourceMatcher::Any ));

        let d: Result<MonitorInputSourceMatcher, serde_yaml::Error> = serde_yaml::from_str("Hdmi1");
        assert!( d.is_ok_and(|input| input == MonitorInputSourceMatcher::Input(libmonitor::mccs::features::InputSource::Hdmi1) ));

        let d: Result<MonitorInputSourceMatcher, serde_yaml::Error> = serde_yaml::from_str("100");
        assert!( d.is_ok_and(|input| input == MonitorInputSourceMatcher::Input(libmonitor::mccs::features::InputSource::Reserved(100)) ));
    }
}

impl<'de> Deserialize<'de> for MonitorInputSourceMatcher {
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
                        Ok(MonitorInputSourceMatcher::Input((num as u32).into()))
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
            Value::String(v) => {
                if v == "Any" {
                    Ok(MonitorInputSourceMatcher::Any)
                } else {
                    Ok(MonitorInputSourceMatcher::Input(InputSource::deserialize(value).map_err(|err| Error::custom(err))?))
                }
            }
            Value::Null => {
                Ok(MonitorInputSourceMatcher::Any)
            }
            _ => {
                Err(Error::custom("expected value of type u8 or variants: Any,<InputSourceVariant>"))
            }
        }
    }
}
