use std::convert::{TryFrom, TryInto};

use super::{ConvertError, FromMqttBytes, ToMqttBytes, VBI};

#[derive(Debug)]
pub struct Puback {
    properties: Vec<properties::PubackProperty>,
    pktid: u16,
    reason: PubackReason,
}

impl Puback {
    pub(crate) fn new(pktid: u16) -> Self {
        Self {
            pktid,
            reason: PubackReason::Success,
            properties: vec![],
        }
    }

    pub(crate) fn pktid(&self) -> u16 {
        self.pktid
    }
}

impl ToMqttBytes for Puback {
    fn convert_to_mqtt(&self) -> Vec<u8> {
        let first_byte: u8 = 0b0100_0000;
        let mut header: Vec<u8> = vec![first_byte];
        let mut buf = vec![];

        buf.extend_from_slice(&self.pktid.to_be_bytes());
        buf.push(self.reason.into());

        buf.extend_from_slice(&self.properties.as_slice().convert_to_mqtt());

        let len_vbi = VBI(buf.len() as u32).convert_to_mqtt();
        header.extend_from_slice(&len_vbi);
        header.extend_from_slice(&buf);

        header
    }
}

impl FromMqttBytes for Puback {
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

        let reason: PubackReason = if remaining_len.as_u32() == 2 {
            0.try_into()?
        } else {
            let reason = bytes[bytes_read].try_into()?;
            bytes_read += 1;
            reason
        };

        let properties = if remaining_len.as_u32() < 4 {
            vec![]
        } else {
            let (properties, props_bytes_read) =
                Vec::<properties::PubackProperty>::convert_from_mqtt(&bytes[bytes_read..])?;
            bytes_read += props_bytes_read;
            properties
        };

        let puback = Self {
            properties,
            pktid,
            reason,
        };
        Ok((puback, bytes_read))
    }
}

#[derive(Debug, Copy, Clone)]
enum PubackReason {
    Success,
    NoMatchingSubscibers,
    UnspecifiedError,
    ImplementationSpecificError,
    NotAuthorized,
    TopicNameInvalid,
    PacketIdentifierInUse,
    QuotaExceeded,
    PayloadFormatInvalid,
}

impl TryFrom<u8> for PubackReason {
    type Error = String;

    fn try_from(b: u8) -> Result<Self, Self::Error> {
        match b {
            0 => Ok(PubackReason::Success),
            16 => Ok(PubackReason::NoMatchingSubscibers),
            128 => Ok(PubackReason::UnspecifiedError),
            131 => Ok(PubackReason::ImplementationSpecificError),
            135 => Ok(PubackReason::NotAuthorized),
            144 => Ok(PubackReason::TopicNameInvalid),
            145 => Ok(PubackReason::PacketIdentifierInUse),
            151 => Ok(PubackReason::QuotaExceeded),
            153 => Ok(PubackReason::PayloadFormatInvalid),
            _ => Err(format!("Failed to parse PubackReason, byte = {}", b)),
        }
    }
}

impl From<PubackReason> for u8 {
    fn from(r: PubackReason) -> Self {
        match r {
            PubackReason::Success => 0,
            PubackReason::NoMatchingSubscibers => 16,
            PubackReason::UnspecifiedError => 128,
            PubackReason::ImplementationSpecificError => 131,
            PubackReason::NotAuthorized => 135,
            PubackReason::TopicNameInvalid => 144,
            PubackReason::PacketIdentifierInUse => 145,
            PubackReason::QuotaExceeded => 151,
            PubackReason::PayloadFormatInvalid => 153,
        }
    }
}

mod properties {
    use super::super::{ConvertError, FromMqttBytes, ToMqttBytes, VBI};
    use PubackProperty::*;

    #[derive(Debug, Clone)]
    pub enum PubackProperty {
        ReasonString(String),
        UserProperty(String, String),
    }

    impl PubackProperty {
        fn id(&self) -> u8 {
            match self {
                ReasonString(_) => 0x1F,
                UserProperty(_, _) => 0x26,
            }
        }
    }

    impl ToMqttBytes for PubackProperty {
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

    impl FromMqttBytes for PubackProperty {
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

    impl ToMqttBytes for &[PubackProperty] {
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

    impl FromMqttBytes for Vec<PubackProperty> {
        fn convert_from_mqtt(bytes: &[u8]) -> Result<(Self, usize), ConvertError> {
            let (bytelen, bytes_consumed) = VBI::convert_from_mqtt(&bytes)?;
            if bytelen.as_u32() == 0 {
                Ok((vec![], bytes_consumed))
            } else {
                let mut bytes =
                    &bytes[bytes_consumed..(bytes_consumed + bytelen.as_u32() as usize)];
                let mut properties = vec![];
                while bytes.len() > 0 {
                    let (prop, bytes_read) = PubackProperty::convert_from_mqtt(bytes)?;
                    properties.push(prop);
                    bytes = &bytes[bytes_read..];
                }

                Ok((properties, bytes_consumed + bytelen.as_u32() as usize))
            }
        }
    }
}
