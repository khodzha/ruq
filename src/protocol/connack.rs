use super::ConvertError;
use super::FromMqttBytes;
use super::VBI;

use std::convert::{TryFrom, TryInto};

#[derive(Debug)]
pub struct Connack {
    flags: ConnackFlags,
    reason: ConnackReason,
    properties: Vec<properties::ConnackProperty>,
}
#[derive(Debug)]
pub struct ConnackFlags {
    pub session_present: bool,
}

impl TryFrom<u8> for ConnackFlags {
    type Error = String;

    fn try_from(b: u8) -> Result<Self, Self::Error> {
        match b {
            0b0000_0000 => Ok(Self {
                session_present: false,
            }),
            0b0000_0001 => Ok(Self {
                session_present: true,
            }),
            _ => Err(format!("Invalid connack flags byte, got = {}", b)),
        }
    }
}

#[derive(Debug)]
pub enum ConnackReason {
    Success,
    UnspecifiedError,
    MalformedPacket,
    ProtocolError,
    ImplementationSpecificError,
    UnsupportedProtocolVersion,
    ClientIdentifierNotValid,
    BadUserNameOrPassword,
    NotAuthorized,
    ServerUnavailable,
    ServerBusy,
    Banned,
    BadAuthenticationMethod,
    TopicNameInvalid,
    PacketTooLarge,
    QuotaExceeded,
    PayloadFormatInvalid,
    RetainNotSupported,
    QoSNotSupported,
    UseAnotherServer,
    ServerMoved,
    ConnectionRateExceeded,
}

impl TryFrom<u8> for ConnackReason {
    type Error = String;

    fn try_from(b: u8) -> Result<Self, Self::Error> {
        match b {
            0 => Ok(Self::Success),
            128 => Ok(Self::UnspecifiedError),
            129 => Ok(Self::MalformedPacket),
            130 => Ok(Self::ProtocolError),
            131 => Ok(Self::ImplementationSpecificError),
            132 => Ok(Self::UnsupportedProtocolVersion),
            133 => Ok(Self::ClientIdentifierNotValid),
            134 => Ok(Self::BadUserNameOrPassword),
            135 => Ok(Self::NotAuthorized),
            136 => Ok(Self::ServerUnavailable),
            137 => Ok(Self::ServerBusy),
            138 => Ok(Self::Banned),
            140 => Ok(Self::BadAuthenticationMethod),
            144 => Ok(Self::TopicNameInvalid),
            149 => Ok(Self::PacketTooLarge),
            151 => Ok(Self::QuotaExceeded),
            153 => Ok(Self::PayloadFormatInvalid),
            154 => Ok(Self::RetainNotSupported),
            155 => Ok(Self::QoSNotSupported),
            156 => Ok(Self::UseAnotherServer),
            157 => Ok(Self::ServerMoved),
            159 => Ok(Self::ConnectionRateExceeded),
            _ => Err(format!("Failed to parse ConnackReason, byte = {}", b)),
        }
    }
}

impl FromMqttBytes for Connack {
    fn convert_from_mqtt(bytes: &[u8]) -> Result<(Self, usize), ConvertError> {
        // Skip packet type
        let mut bytes_read = 1;

        let (remaining_len, vbi_bytes_read) = VBI::convert_from_mqtt(&bytes[bytes_read..])?;
        bytes_read += vbi_bytes_read;

        if bytes.len() < remaining_len.as_u32() as usize + bytes_read {
            return Err(ConvertError::NotEnoughBytes);
        }

        let flags: ConnackFlags = bytes[bytes_read].try_into()?;
        bytes_read += 1;

        let reason: ConnackReason = bytes[bytes_read].try_into()?;
        bytes_read += 1;

        let (properties, props_bytes_read) =
            Vec::<properties::ConnackProperty>::convert_from_mqtt(&bytes[bytes_read..])?;
        bytes_read += props_bytes_read;

        let connack = Self {
            properties,
            flags,
            reason,
        };
        Ok((connack, bytes_read))
    }
}

mod properties {
    use super::super::{ConvertError, FromMqttBytes, ToMqttBytes, VBI};
    use ConnackProperty::*;

    #[derive(Debug, Clone)]
    pub enum ConnackProperty {
        // SessionExpiryInterval(u32),
        // ReceiveMaximum(u16),
        // MaximumQoS(u8), // QoS instead?
        // RetainAvailable(bool),
        // MaximumPacketSize(u32),
        // AssignedClientId(String),
        // TopicAliasMaximum(u16)
        ReasonString(String),
        UserProperty(String, String),
        // WildcardSubscriptionAvailable(bool),
        // SubscriptionIdentifiersAvailable(bool),
        // SharedSubscriptionAvailable(bool),
        // ServerKeepAlive(u16),
        // ResponseInformation(String),
        // ServerReference(String),
        // AuthenticationMethod(String),
        // AuthenticationData(String)
    }

    impl ConnackProperty {
        fn id(&self) -> u8 {
            match self {
                ReasonString(_) => 0x1F,
                UserProperty(_, _) => 0x26,
            }
        }
    }

    impl ToMqttBytes for ConnackProperty {
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

    impl FromMqttBytes for ConnackProperty {
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

    impl ToMqttBytes for &[ConnackProperty] {
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

    impl FromMqttBytes for Vec<ConnackProperty> {
        fn convert_from_mqtt(bytes: &[u8]) -> Result<(Self, usize), ConvertError> {
            let (bytelen, bytes_consumed) = VBI::convert_from_mqtt(&bytes)?;
            if bytelen.as_u32() == 0 {
                Ok((vec![], bytes_consumed))
            } else {
                let mut bytes =
                    &bytes[bytes_consumed..(bytes_consumed + bytelen.as_u32() as usize)];
                let mut properties = vec![];
                while bytes.len() > 0 {
                    let (prop, bytes_read) = ConnackProperty::convert_from_mqtt(bytes)?;
                    properties.push(prop);
                    bytes = &bytes[bytes_read..];
                }

                Ok((properties, bytes_consumed + bytelen.as_u32() as usize))
            }
        }
    }
}
