use std::convert::{TryFrom, TryInto};

use super::{ConvertError, FromMqttBytes, ToMqttBytes, VBI};

#[derive(Debug)]
pub struct Suback {
    properties: Vec<properties::SubackProperty>,
    pktid: u16,
    reason: SubackReason,
}

impl FromMqttBytes for Suback {
    fn convert_from_mqtt(bytes: &[u8]) -> Result<(Self, usize), ConvertError> {
        // Skip packet type
        let mut bytes_read = 1;

        let (remaining_len, vbi_bytes_read) = VBI::convert_from_mqtt(&bytes[bytes_read..])?;
        bytes_read += vbi_bytes_read;

        if bytes.len() < remaining_len.as_u32() as usize + bytes_read {
            return Err(ConvertError::NotEnoughBytes);
        }

        let pktid = {
            let pktid = bytes[bytes_read..]
                .get(0..2)
                .ok_or(ConvertError::NotEnoughBytes)
                .and_then(|slice| {
                    slice
                        .try_into()
                        .map_err(|e| format!("Failed to convert to u16, reason = {:?}", e).into())
                })
                .map(|slice| u16::from_be_bytes(slice))?;

            bytes_read += 2;
            pktid
        };

        let (properties, props_bytes_read) =
            Vec::<properties::SubackProperty>::convert_from_mqtt(&bytes[bytes_read..])?;
        bytes_read += props_bytes_read;

        let reason: SubackReason = bytes[bytes_read].try_into()?;
        bytes_read += 1;

        let suback = Self {
            properties,
            pktid,
            reason,
        };
        Ok((suback, bytes_read))
    }
}

#[derive(Debug, Copy, Clone)]
enum SubackReason {
    QoS0,
    QoS1,
    QoS2,
    UnspecifiedError,
    ImplementationSpecificError,
    NotAuthorized,
    TopicFilterInvalid,
    PacketIdentifierInUse,
    QuotaExceeded,
    SharedSubsNotSupported,
    SubIdsNotSupported,
    WildcardSubsNotSupported,
}

impl TryFrom<u8> for SubackReason {
    type Error = String;

    fn try_from(b: u8) -> Result<Self, Self::Error> {
        match b {
            0 => Ok(Self::QoS0),
            1 => Ok(Self::QoS1),
            2 => Ok(Self::QoS2),
            128 => Ok(Self::UnspecifiedError),
            131 => Ok(Self::ImplementationSpecificError),
            135 => Ok(Self::NotAuthorized),
            143 => Ok(Self::TopicFilterInvalid),
            145 => Ok(Self::PacketIdentifierInUse),
            151 => Ok(Self::QuotaExceeded),
            158 => Ok(Self::SharedSubsNotSupported),
            161 => Ok(Self::SubIdsNotSupported),
            162 => Ok(Self::WildcardSubsNotSupported),
            _ => Err(format!("Failed to parse SubackReason, byte = {}", b)),
        }
    }
}

mod properties {
    use super::super::{ConvertError, FromMqttBytes, ToMqttBytes, VBI};
    use SubackProperty::*;

    #[derive(Debug, Clone)]
    pub enum SubackProperty {
        ReasonString(String),
        UserProperty(String, String),
    }

    impl SubackProperty {
        fn id(&self) -> u8 {
            match self {
                ReasonString(_) => 0x1F,
                UserProperty(_, _) => 0x26,
            }
        }
    }

    impl ToMqttBytes for SubackProperty {
        fn convert_to_mqtt(&self) -> Vec<u8> {
            let first_byte: u8 = self.id();
            let mut buf: Vec<u8> = vec![first_byte];
            match self {
                ReasonString(s) => {
                    buf.extend_from_slice(&s.convert_to_mqtt());
                }
                UserProperty(k, v) => {
                    buf.extend_from_slice(&k.convert_to_mqtt());
                    buf.extend_from_slice(&v.convert_to_mqtt());
                }
            }

            buf
        }
    }

    impl FromMqttBytes for SubackProperty {
        fn convert_from_mqtt(bytes: &[u8]) -> Result<(Self, usize), ConvertError> {
            match bytes.get(0) {
                Some(0x1F) => {
                    let (s, bytes_consumed) = String::convert_from_mqtt(&bytes[1..])?;
                    Ok((ReasonString(s), bytes_consumed + 1))
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

    impl ToMqttBytes for &[SubackProperty] {
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

    impl FromMqttBytes for Vec<SubackProperty> {
        fn convert_from_mqtt(bytes: &[u8]) -> Result<(Self, usize), ConvertError> {
            let (bytelen, bytes_consumed) = VBI::convert_from_mqtt(&bytes)?;
            if bytelen.as_u32() == 0 {
                Ok((vec![], bytes_consumed))
            } else {
                let mut bytes =
                    &bytes[bytes_consumed..(bytes_consumed + bytelen.as_u32() as usize)];
                let mut properties = vec![];
                while bytes.len() > 0 {
                    let (prop, bytes_read) = SubackProperty::convert_from_mqtt(bytes)?;
                    properties.push(prop);
                    bytes = &bytes[bytes_read..];
                }

                Ok((properties, bytes_consumed + bytelen.as_u32() as usize))
            }
        }
    }
}
