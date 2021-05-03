use std::convert::TryFrom;

use super::ConvertError;

#[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum QoS {
    AtMostOnce,
    AtLeastOnce,
    ExactlyOnce,
}

impl From<QoS> for u8 {
    fn from(qos: QoS) -> u8 {
        match qos {
            QoS::AtMostOnce => 0,
            QoS::AtLeastOnce => 1,
            QoS::ExactlyOnce => 2,
        }
    }
}

impl TryFrom<u8> for QoS {
    type Error = ConvertError;

    fn try_from(u: u8) -> Result<Self, Self::Error> {
        match u {
            0 => Ok(QoS::AtMostOnce),
            1 => Ok(QoS::AtLeastOnce),
            2 => Ok(QoS::ExactlyOnce),
            _ => Err(ConvertError::Other(format!("Invalid qos value: {}", u))),
        }
    }
}
