use std::convert::{TryFrom, TryInto};

use super::{ConvertError, FromMqttBytes, PktId, ToMqttBytes, VBI};

#[derive(Debug)]
pub struct Puback {
    properties: properties::PubackProperties,
    pktid: PktId,
    reason: PubackReason,
}

impl Puback {
    pub(crate) fn new(pktid: PktId) -> Self {
        Self {
            pktid,
            reason: PubackReason::Success,
            properties: Default::default(),
        }
    }

    pub(crate) fn pktid(&self) -> PktId {
        self.pktid
    }
}

impl ToMqttBytes for Puback {
    fn convert_to_mqtt(&self) -> Vec<u8> {
        let first_byte: u8 = 0b0100_0000;
        let mut header: Vec<u8> = vec![first_byte];
        let mut buf = vec![];

        buf.extend_from_slice(&self.pktid.get().to_be_bytes());
        buf.push(self.reason.into());

        buf.extend_from_slice(&self.properties.convert_to_mqtt());

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

        if bytes.len() < remaining_len + bytes_read {
            return Err(ConvertError::NotEnoughBytes);
        }

        let (pktid, pktid_bytes_read) = PktId::convert_from_mqtt(&bytes[bytes_read..])?;
        bytes_read += pktid_bytes_read;

        let reason: PubackReason = if remaining_len == 2 {
            0.try_into()?
        } else {
            let reason = bytes[bytes_read].try_into()?;
            bytes_read += 1;
            reason
        };

        let properties = if remaining_len < 4 {
            Default::default()
        } else {
            let (properties, props_bytes_read) =
                properties::PubackProperties::convert_from_mqtt(&bytes[bytes_read..])?;
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

    #[derive(Debug, Clone, Default)]
    pub struct PubackProperties {
        reason_string: Option<String>,
        user_properties: Vec<(String, String)>,
    }

    impl ToMqttBytes for PubackProperties {
        fn convert_to_mqtt(&self) -> Vec<u8> {
            let mut buf: Vec<u8> = vec![];
            if let Some(s) = &self.reason_string {
                buf.push(0x1F);
                buf.extend_from_slice(&s.convert_to_mqtt());
            }

            if self.user_properties.len() > 0 {
                buf.push(0x26);
            }
            for (k, v) in &self.user_properties {
                buf.extend_from_slice(&k.convert_to_mqtt());
                buf.extend_from_slice(&v.convert_to_mqtt());
            }

            buf
        }
    }

    impl FromMqttBytes for PubackProperties {
        fn convert_from_mqtt(bytes: &[u8]) -> Result<(Self, usize), ConvertError> {
            let (bytelen, bytes_consumed) = VBI::convert_from_mqtt(&bytes)?;
            if bytelen == 0 {
                Ok((Default::default(), bytes_consumed))
            } else {
                let mut bytes = bytes.get(bytes_consumed..(bytes_consumed + bytelen));

                let mut properties: PubackProperties = Default::default();

                while let Some(slice) = bytes {
                    let bytes_read = match slice.get(0) {
                        Some(0x1F) => {
                            let (s, property_bytes_read) = String::convert_from_mqtt(&slice[1..])?;
                            properties.reason_string = Some(s);
                            property_bytes_read + 1
                        }
                        Some(0x26) => {
                            let (k, pbr1) = String::convert_from_mqtt(&slice[1..])?;
                            let (v, pbr2) = String::convert_from_mqtt(&slice[(pbr1 + 1)..])?;
                            properties.user_properties.push((k, v));
                            pbr1 + pbr2 + 1
                        }
                        Some(k) => {
                            return Err(ConvertError::Other(format!(
                                "Property {:#x?} not implemented",
                                k
                            )));
                        }
                        None => {
                            return Err(ConvertError::NotEnoughBytes);
                        }
                    };
                    bytes = slice.get((bytes_read + 1)..);
                }

                Ok((properties, bytes_consumed + bytelen))
            }
        }
    }

    #[test]
    fn empty_puback_props() {
        let (props, _) = PubackProperties::convert_from_mqtt(&[0]).unwrap();
        assert!(props.reason_string.is_none());
        assert!(props.user_properties.len() == 0);
    }

    #[test]
    fn some_reason_string() {
        let bytes = [9, 0x1F, 0, 6, b'f', b'o', b'o', b'b', b'a', b'r'];
        let (props, _) = PubackProperties::convert_from_mqtt(&bytes).unwrap();
        assert!(props.reason_string.unwrap() == "foobar");
        assert!(props.user_properties.len() == 0);
    }

    #[test]
    fn malformed_reason_string() {
        let bytes = [9, 0x1F, 6, 0, b'f', b'o', b'o', b'b', b'a', b'r'];
        let (props, _) = PubackProperties::convert_from_mqtt(&bytes).unwrap();
        assert!(props.reason_string.unwrap() == "foobar");
        assert!(props.user_properties.len() == 0);
    }

    #[test]
    fn some_user_props_string() {
        let bytes = [
            12, 0x26, 0, 3, b'f', b'o', b'o', 0, 4, b'b', b'a', b'r', b'k',
        ];
        let (props, _) = PubackProperties::convert_from_mqtt(&bytes).unwrap();
        assert!(props.reason_string.is_none());
        assert!(props.user_properties.len() == 1);
        assert!(props.user_properties[0] == ("foo".into(), "bark".into()));
    }
}
