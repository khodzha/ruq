use std::convert::TryInto;

use bytes::BufMut;
use Property::*;

use super::{ConvertError, FromMqttBytes, ToMqttBytes, VBI};

#[derive(Debug, Clone)]
pub enum Property {
    PayloadFormatId(u8),
    MessageExpiryInterval(u32),
    //TopicAlias(u16),
    ResponseTopic(String),
    CorrelationData(Vec<u8>),
    UserProperty(String, String),
    //SubscriptionIdentifier(VBI),
    //ContentType(String),
}

impl Property {
    fn id(&self) -> u8 {
        match self {
            PayloadFormatId(_) => 0x01,
            MessageExpiryInterval(_) => 0x02,
            ResponseTopic(_) => 0x08,
            CorrelationData(_) => 0x09,
            UserProperty(_, _) => 0x26,
        }
    }
}

impl ToMqttBytes for Property {
    fn convert_to_mqtt(&self) -> Vec<u8> {
        let first_byte: u8 = self.id();
        let mut buf: Vec<u8> = vec![first_byte];
        match self {
            PayloadFormatId(v) => buf.put_u8(*v),
            MessageExpiryInterval(v) => {
                buf.put_u32(*v);
            }
            ResponseTopic(s) => {
                buf.extend_from_slice(&s.convert_to_mqtt());
            }
            CorrelationData(b) => {
                buf.extend_from_slice(&b.as_slice().convert_to_mqtt());
            }
            UserProperty(k, v) => {
                buf.extend_from_slice(&k.convert_to_mqtt());
                buf.extend_from_slice(&v.convert_to_mqtt());
            }
        }

        buf
    }
}

impl FromMqttBytes for Property {
    fn convert_from_mqtt(bytes: &[u8]) -> Result<(Self, usize), ConvertError> {
        match bytes.get(0) {
            Some(0x01) => Ok((PayloadFormatId(bytes[1]), 2)),
            Some(0x02) => {
                let interval = bytes
                    .get(1..5)
                    .ok_or(ConvertError::NotEnoughBytes)
                    .and_then(|slice| {
                        slice.try_into().map_err(|e| {
                            format!("Failed to convert to u32, reason = {:?}", e).into()
                        })
                    })
                    .map(|slice| u32::from_be_bytes(slice))?;
                Ok((MessageExpiryInterval(interval), 4))
            }
            Some(0x08) => {
                let (s, bytes_consumed) = String::convert_from_mqtt(&bytes[1..])?;
                Ok((ResponseTopic(s), bytes_consumed + 1))
            }
            Some(0x09) => {
                let (s, bytes_consumed) = Vec::<u8>::convert_from_mqtt(&bytes[1..])?;
                Ok((CorrelationData(s), bytes_consumed + 1))
            }
            Some(0x26) => {
                let (k, bc1) = String::convert_from_mqtt(&bytes[1..])?;
                let (v, bc2) = String::convert_from_mqtt(&bytes[(bc1 + 1)..])?;
                Ok((UserProperty(k, v), bc1 + bc2 + 1))
            }
            Some(k) => Err(ConvertError::Other(format!(
                "Property {:#x?} not implemented",
                k
            ))),
            None => Err(ConvertError::NotEnoughBytes),
        }
    }
}

impl ToMqttBytes for &[Property] {
    fn convert_to_mqtt(&self) -> Vec<u8> {
        let mut header = vec![];
        let mut buf: Vec<u8> = vec![];

        self.iter()
            .for_each(|prop| buf.extend_from_slice(&prop.convert_to_mqtt()));

        let len: VBI = (buf.len() as u32).into();
        header.extend_from_slice(&len.convert_to_mqtt());
        header.extend_from_slice(&buf);
        header
    }
}

impl FromMqttBytes for Vec<Property> {
    fn convert_from_mqtt(bytes: &[u8]) -> Result<(Self, usize), ConvertError> {
        let (bytelen, bytes_consumed) = VBI::convert_from_mqtt(&bytes)?;
        if bytelen.as_u32() == 0 {
            Ok((vec![], bytes_consumed))
        } else {
            let mut bytes = &bytes[bytes_consumed..(bytes_consumed + bytelen.as_u32() as usize)];
            let mut properties = vec![];
            while bytes.len() > 0 {
                let (prop, bytes_read) = Property::convert_from_mqtt(bytes)?;
                properties.push(prop);
                bytes = &bytes[bytes_read..];
            }

            Ok((properties, bytes_consumed + bytelen.as_u32() as usize))
        }
    }
}
