use super::{ConvertError, FromMqttBytes, ToMqttBytes, VBI};
use std::convert::{TryFrom, TryInto};

#[derive(Debug)]
pub struct Disconnect {
    reason: DisconnectReason,
    properties: Vec<properties::DisconnectProperty>,
}

impl Disconnect {
    pub fn new() -> Self {
        Self {
            reason: Normal,
            properties: vec![],
        }
    }

    pub fn set_reason(&mut self, reason: DisconnectReason) -> &Self {
        self.reason = reason;
        self
    }

    pub fn set_property(&mut self, property: properties::DisconnectProperty) {
        self.properties.push(property);
    }
}

#[derive(Debug)]
pub enum DisconnectReason {
    Normal,
    DisconnectWithWill,
    UnspecifiedError,
    MalformedPacket,
    ProtocolError,
    ImplementationSpecificError,
    NotAuthorized,
    ServerBusy,
    ServerShuttingDown,
    KeepAliveTimeout,
    SessionTakenOver,
    TopicFilterInvalid,
    TopicNameInvalid,
    ReceiveMaximumExceeded,
    TopicAliasInvalid,
    PacketTooLarge,
    MessageRateTooHigh,
    QuotaExceeded,
    AdministrativeAction,
    PayloadFormatInvalid,
    RetainNotSupported,
    QoSNotSupported,
    UseAnotherServer,
    ServerMoved,
    SharedSubsNotSupported,
    ConnectionRateExceeded,
    MaximumConnectTime,
    SubscrpitionIdsNotSupported,
    WildcardSubsNotSupported,
}

use DisconnectReason::*;

impl TryFrom<u8> for DisconnectReason {
    type Error = String;

    fn try_from(b: u8) -> Result<Self, Self::Error> {
        match b {
            0 => Ok(Normal),
            4 => Ok(DisconnectWithWill),
            128 => Ok(UnspecifiedError),
            129 => Ok(MalformedPacket),
            130 => Ok(ProtocolError),
            131 => Ok(ImplementationSpecificError),
            135 => Ok(NotAuthorized),
            137 => Ok(ServerBusy),
            139 => Ok(ServerShuttingDown),
            141 => Ok(KeepAliveTimeout),
            142 => Ok(SessionTakenOver),
            143 => Ok(TopicFilterInvalid),
            144 => Ok(TopicNameInvalid),
            147 => Ok(ReceiveMaximumExceeded),
            148 => Ok(TopicAliasInvalid),
            149 => Ok(PacketTooLarge),
            150 => Ok(MessageRateTooHigh),
            151 => Ok(QuotaExceeded),
            152 => Ok(AdministrativeAction),
            153 => Ok(PayloadFormatInvalid),
            154 => Ok(RetainNotSupported),
            155 => Ok(QoSNotSupported),
            156 => Ok(UseAnotherServer),
            157 => Ok(ServerMoved),
            158 => Ok(SharedSubsNotSupported),
            159 => Ok(ConnectionRateExceeded),
            160 => Ok(MaximumConnectTime),
            161 => Ok(SubscrpitionIdsNotSupported),
            162 => Ok(WildcardSubsNotSupported),
            _ => Err(format!("Failed to parse DisconnectReason, byte = {}", b)),
        }
    }
}

impl From<DisconnectReason> for u8 {
    fn from(b: DisconnectReason) -> Self {
        match b {
            Normal => 0,
            DisconnectWithWill => 4,
            UnspecifiedError => 128,
            MalformedPacket => 129,
            ProtocolError => 130,
            ImplementationSpecificError => 131,
            NotAuthorized => 135,
            ServerBusy => 137,
            ServerShuttingDown => 139,
            KeepAliveTimeout => 141,
            SessionTakenOver => 142,
            TopicFilterInvalid => 143,
            TopicNameInvalid => 144,
            ReceiveMaximumExceeded => 147,
            TopicAliasInvalid => 148,
            PacketTooLarge => 149,
            MessageRateTooHigh => 150,
            QuotaExceeded => 151,
            AdministrativeAction => 152,
            PayloadFormatInvalid => 153,
            RetainNotSupported => 154,
            QoSNotSupported => 155,
            UseAnotherServer => 156,
            ServerMoved => 157,
            SharedSubsNotSupported => 158,
            ConnectionRateExceeded => 159,
            MaximumConnectTime => 160,
            SubscrpitionIdsNotSupported => 161,
            WildcardSubsNotSupported => 162,
        }
    }
}

impl ToMqttBytes for Disconnect {
    fn convert_to_mqtt(&self) -> Vec<u8> {
        let mut header: Vec<u8> = vec![0b1110_0000];
        let mut buf = vec![];

        buf.extend_from_slice(&self.properties.as_slice().convert_to_mqtt());

        // empty payload

        let len_vbi = VBI(buf.len() as u32).convert_to_mqtt();
        header.extend_from_slice(&len_vbi);
        header.extend_from_slice(&buf);

        header
    }
}

impl FromMqttBytes for Disconnect {
    fn convert_from_mqtt(bytes: &[u8]) -> Result<(Self, usize), ConvertError> {
        // Skip packet type
        let mut bytes_read = 1;

        let (remaining_len, vbi_bytes_read) = VBI::convert_from_mqtt(&bytes[bytes_read..])?;
        bytes_read += vbi_bytes_read;

        if bytes.len() < remaining_len.as_u32() as usize + bytes_read {
            return Err(ConvertError::NotEnoughBytes);
        }

        let reason: DisconnectReason = if remaining_len.as_u32() == 0 {
            DisconnectReason::Normal
        } else {
            let reason = bytes[bytes_read].try_into()?;
            bytes_read += 1;
            reason
        };

        let properties = if remaining_len.as_u32() < 4 {
            vec![]
        } else {
            let (properties, props_bytes_read) =
                Vec::<properties::DisconnectProperty>::convert_from_mqtt(&bytes[bytes_read..])?;
            bytes_read += props_bytes_read;
            properties
        };

        let disconnect = Self { properties, reason };
        Ok((disconnect, bytes_read))
    }
}

mod properties {
    use super::super::{ConvertError, FromMqttBytes, ToMqttBytes, VBI};
    use DisconnectProperty::*;

    #[derive(Debug, Clone)]
    pub enum DisconnectProperty {
        SessionExpiryInterval(u32),
        ReasonString(String),
        UserProperty(String, String),
    }

    impl DisconnectProperty {
        fn id(&self) -> u8 {
            match self {
                SessionExpiryInterval(_) => 0x11,
                ReasonString(_) => 0x1F,
                UserProperty(_, _) => 0x26,
            }
        }
    }

    impl ToMqttBytes for DisconnectProperty {
        fn convert_to_mqtt(&self) -> Vec<u8> {
            let first_byte: u8 = self.id();
            let mut buf: Vec<u8> = vec![first_byte];
            match self {
                SessionExpiryInterval(v) => {
                    buf.extend_from_slice(&v.to_be_bytes());
                }
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

    impl FromMqttBytes for DisconnectProperty {
        fn convert_from_mqtt(bytes: &[u8]) -> Result<(Self, usize), ConvertError> {
            match bytes.get(0) {
                Some(0x11) => Err(ConvertError::Other(format!(
                    "Property SessionExpiryInterval must not be sent by server",
                ))),
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

    impl ToMqttBytes for &[DisconnectProperty] {
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

    impl FromMqttBytes for Vec<DisconnectProperty> {
        fn convert_from_mqtt(bytes: &[u8]) -> Result<(Self, usize), ConvertError> {
            let (bytelen, bytes_consumed) = VBI::convert_from_mqtt(&bytes)?;
            if bytelen.as_u32() == 0 {
                Ok((vec![], bytes_consumed))
            } else {
                let mut bytes =
                    &bytes[bytes_consumed..(bytes_consumed + bytelen.as_u32() as usize)];
                let mut properties = vec![];
                while bytes.len() > 0 {
                    let (prop, bytes_read) = DisconnectProperty::convert_from_mqtt(bytes)?;
                    properties.push(prop);
                    bytes = &bytes[bytes_read..];
                }

                Ok((properties, bytes_consumed + bytelen.as_u32() as usize))
            }
        }
    }
}
