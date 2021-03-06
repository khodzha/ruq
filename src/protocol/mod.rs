use std::{
    convert::{TryFrom, TryInto},
    num::NonZeroU16,
};

pub use connack::Connack;
pub use connect::Connect;
pub use disconnect::{Disconnect, DisconnectReason};
pub use pingreq::PingReq;
pub use pingresp::PingResp;
pub use properties::Property;
pub use puback::Puback;
pub(crate) use publish::Publish;
pub use qos::QoS;
pub use suback::Suback;
pub use subscribe::Subscribe;
pub use unsuback::Unsuback;
pub use unsubscribe::Unsubscribe;

use log::{error, info};

pub fn parse_pkt(data: &[u8]) -> Result<Option<Packet>, std::io::Error> {
    let pkt_type = parse_fixed_header(data[0]).map_err(|e| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "Byte {:?} is not a proper packet header, err = {:?}",
                data[0], e
            ),
        )
    })?;

    let pkt = match pkt_type {
        PacketType::Publish => {
            Publish::convert_from_mqtt(&data).map(|(pkt, _)| Some(Packet::Publish(pkt)))
        }
        PacketType::Connack => {
            Connack::convert_from_mqtt(&data).map(|(pkt, _)| Some(Packet::Connack(pkt)))
        }
        PacketType::PingResponse => {
            PingResp::convert_from_mqtt(&data).map(|(pkt, _)| Some(Packet::PingResp(pkt)))
        }
        PacketType::SubscribeAck => {
            Suback::convert_from_mqtt(&data).map(|(pkt, _)| Some(Packet::SubAck(pkt)))
        }
        PacketType::UnsubscribeAck => {
            Unsuback::convert_from_mqtt(&data).map(|(pkt, _)| Some(Packet::UnsubAck(pkt)))
        }
        PacketType::PubAck => {
            Puback::convert_from_mqtt(&data).map(|(pkt, _)| Some(Packet::PubAck(pkt)))
        }
        PacketType::Disconnect => {
            Disconnect::convert_from_mqtt(&data).map(|(pkt, _)| Some(Packet::Disconnect(pkt)))
        }
        ty => {
            info!("Received msg type = {:?}", ty);
            unimplemented!("This is not impl");
        }
    };

    pkt.map_err(|e| match e {
        ConvertError::NotEnoughBytes => {
            info!("Received publish but not enough bytes?");
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("parse_pkt expects that full pkt is present"),
            )
        }
        e => {
            error!(
                "Something went wrong with parsing publish pkt, err = {:?}",
                e
            );
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "Something went wrong with parsing publish pkt, err = {:?}",
                    e
                ),
            )
        }
    })
}

#[derive(Debug)]
pub enum Packet {
    Connect(Connect),
    Connack(Connack),
    Publish(Publish),
    PubAck(Puback),
    PingReq(PingReq),
    PingResp(PingResp),
    Subscribe(Subscribe),
    SubAck(Suback),
    Unsubscribe(Unsubscribe),
    UnsubAck(Unsuback),
    Disconnect(Disconnect),
    /* TODO:
    PubReceived,
    PubRelease,
    PubComplete,
    Auth,
    */
}

impl ToMqttBytes for Packet {
    fn convert_to_mqtt(&self) -> Vec<u8> {
        match self {
            Self::Connect(p) => p.convert_to_mqtt(),
            Self::Publish(p) => p.convert_to_mqtt(),
            Self::PingReq(p) => p.convert_to_mqtt(),
            Self::Subscribe(p) => p.convert_to_mqtt(),
            Self::Unsubscribe(p) => p.convert_to_mqtt(),
            Self::Disconnect(p) => p.convert_to_mqtt(),
            Self::PingResp(_) => unimplemented!("Client never sends this"),
            Self::Connack(_) => unimplemented!("Client never sends this"),
            Self::PubAck(_) => unimplemented!("Client never sends this"),
            Self::SubAck(_) => unimplemented!("Client never sends this"),
            Self::UnsubAck(_) => unimplemented!("Client never sends this"),
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum PacketType {
    Connect,
    Connack,
    Publish,
    PubAck,
    PubReceived,
    PubRelease,
    PubComplete,
    Subscribe,
    SubscribeAck,
    Unsubscribe,
    UnsubscribeAck,
    PingRequest,
    PingResponse,
    Disconnect,
    Auth,
}

pub fn parse_fixed_header(byte: u8) -> Result<PacketType, String> {
    byte.try_into()
}

impl TryFrom<u8> for PacketType {
    type Error = String;

    fn try_from(b: u8) -> Result<Self, Self::Error> {
        match b >> 4 {
            1 => Ok(PacketType::Connect),
            2 => Ok(PacketType::Connack),
            3 => Ok(PacketType::Publish),
            4 => Ok(PacketType::PubAck),
            5 => Ok(PacketType::PubReceived),
            6 => Ok(PacketType::PubRelease),
            7 => Ok(PacketType::PubComplete),
            8 => Ok(PacketType::Subscribe),
            9 => Ok(PacketType::SubscribeAck),
            10 => Ok(PacketType::Unsubscribe),
            11 => Ok(PacketType::UnsubscribeAck),
            12 => Ok(PacketType::PingRequest),
            13 => Ok(PacketType::PingResponse),
            14 => Ok(PacketType::Disconnect),
            15 => Ok(PacketType::Auth),
            _ => Err(format!(
                "Failed conversion from u8 to PacketType, byte = {}",
                b
            )),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct VBI(u32);

impl From<u32> for VBI {
    fn from(val: u32) -> Self {
        Self(val)
    }
}

impl From<VBI> for u32 {
    fn from(vbi: VBI) -> Self {
        vbi.0
    }
}

impl PartialEq<u32> for VBI {
    fn eq(&self, other: &u32) -> bool {
        self.0 == *other
    }
}

impl std::cmp::PartialOrd<u32> for VBI {
    fn partial_cmp(&self, other: &u32) -> Option<std::cmp::Ordering> {
        Some(self.0.cmp(other))
    }
}

impl std::cmp::PartialEq<VBI> for u32 {
    fn eq(&self, other: &VBI) -> bool {
        *self == other.0
    }
}

impl std::cmp::PartialOrd<VBI> for u32 {
    fn partial_cmp(&self, other: &VBI) -> Option<std::cmp::Ordering> {
        Some(self.cmp(&other.0))
    }
}

impl std::fmt::Display for VBI {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::ops::Add<usize> for VBI {
    type Output = usize;

    fn add(self, other: usize) -> Self::Output {
        other + self.0 as usize
    }
}

impl std::ops::Add<VBI> for usize {
    type Output = usize;

    fn add(self, other: VBI) -> Self::Output {
        self + other.0 as usize
    }
}

impl std::ops::Sub<usize> for VBI {
    type Output = usize;

    fn sub(self, other: usize) -> Self::Output {
        self.0 as usize - other
    }
}

impl std::ops::Sub<VBI> for usize {
    type Output = usize;

    fn sub(self, other: VBI) -> Self::Output {
        self - other.0 as usize
    }
}

impl ToMqttBytes for VBI {
    fn convert_to_mqtt(&self) -> Vec<u8> {
        let mut n = self.0;
        let mut vec = vec![];
        loop {
            let mut b = (n % 128) as u8;
            n = n / 128;
            if n > 0 {
                b = b | 128;
                vec.push(b);
            } else {
                vec.push(b);
                break;
            }
        }

        vec
    }
}

impl FromMqttBytes for VBI {
    fn convert_from_mqtt(bytes: &[u8]) -> Result<(Self, usize), ConvertError> {
        let mut multiplier = 1;
        let mut value: u32 = 0;
        let mut idx = 0;
        loop {
            match bytes.get(idx) {
                None => return Err(ConvertError::NotEnoughBytes),
                Some(byte) => {
                    value += (byte & 127) as u32 * multiplier;
                    if multiplier > 128 * 128 * 128 {
                        return Err(format!(
                            "Invalid VariableByteInteger exceeds 4 bytes, bytes = {:?}",
                            &bytes[0..3]
                        )
                        .into());
                    }
                    multiplier *= 128;
                    idx += 1;

                    if byte & 128 == 0 {
                        break;
                    }
                }
            }
        }

        Ok((value.into(), idx))
    }
}

#[derive(Debug)]
pub enum ConvertError {
    NotEnoughBytes,
    ZeroPktId,
    Other(String),
}

impl From<ConvertError> for String {
    fn from(e: ConvertError) -> Self {
        format!("ConvertError = {:?}", e)
    }
}

impl From<String> for ConvertError {
    fn from(s: String) -> Self {
        Self::Other(s)
    }
}

impl From<ConvertError> for std::io::Error {
    fn from(e: ConvertError) -> Self {
        std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("ConvertError: something went wrong in proto, e = {:?}", e),
        )
    }
}

pub trait ToMqttBytes: std::fmt::Debug {
    fn convert_to_mqtt(&self) -> Vec<u8>;
}

pub trait FromMqttBytes: Sized {
    fn convert_from_mqtt(bytes: &[u8]) -> Result<(Self, usize), ConvertError>;
}

impl ToMqttBytes for &str {
    fn convert_to_mqtt(&self) -> Vec<u8> {
        let bytes = self.as_bytes();
        let len = bytes.len();
        let mut buf: Vec<u8> = Vec::with_capacity(len + 2);
        buf.extend_from_slice(&(len as u16).to_be_bytes());
        buf.extend_from_slice(bytes);
        buf
    }
}

impl ToMqttBytes for String {
    fn convert_to_mqtt(&self) -> Vec<u8> {
        let bytes = self.as_bytes();
        let len = bytes.len();
        let mut buf: Vec<u8> = Vec::with_capacity(len + 2);
        buf.extend_from_slice(&(len as u16).to_be_bytes());
        buf.extend_from_slice(bytes);
        buf
    }
}

impl ToMqttBytes for &[u8] {
    fn convert_to_mqtt(&self) -> Vec<u8> {
        let len = self.len();
        let mut buf: Vec<u8> = Vec::with_capacity(len + 2);
        buf.extend_from_slice(&(len as u16).to_be_bytes());
        buf.extend_from_slice(self);
        buf
    }
}

impl FromMqttBytes for String {
    fn convert_from_mqtt(bytes: &[u8]) -> Result<(Self, usize), ConvertError> {
        let len = bytes
            .get(0..2)
            .ok_or(ConvertError::NotEnoughBytes)
            .and_then(|slice| {
                slice
                    .try_into()
                    .map_err(|e| format!("Failed to convert to u16, reason = {:?}", e).into())
            })
            .map(|slice| u16::from_be_bytes(slice))? as usize;

        if len > 0 && len + 2 <= bytes.len() {
            let s = std::str::from_utf8(&bytes[2..(2 + len)])
                .map_err(|e| format!("Failed to parse bytes as utf8, reason = {:?}", e))?;

            Ok((s.into(), len + 2))
        } else if len == 0 {
            Ok(("".into(), 2))
        } else {
            Err(ConvertError::NotEnoughBytes)
        }
    }
}

impl FromMqttBytes for Vec<u8> {
    fn convert_from_mqtt(bytes: &[u8]) -> Result<(Self, usize), ConvertError> {
        let len = bytes
            .get(0..2)
            .ok_or(ConvertError::NotEnoughBytes)
            .and_then(|slice| {
                slice
                    .try_into()
                    .map_err(|e| format!("Failed to convert to u16, reason = {:?}", e).into())
            })
            .map(|slice| u16::from_be_bytes(slice))? as usize;
        if len > 0 {
            let v = bytes[2..(2 + len)].to_owned();
            Ok((v, len + 2))
        } else {
            Ok((vec![], 2))
        }
    }
}

pub(crate) type PktId = NonZeroU16;

impl FromMqttBytes for PktId {
    fn convert_from_mqtt(bytes: &[u8]) -> Result<(Self, usize), ConvertError> {
        let id = bytes
            .get(0..2)
            .ok_or(ConvertError::NotEnoughBytes)
            .and_then(|slice| {
                slice
                    .try_into()
                    .map_err(|e| format!("Failed to convert to u16, reason = {:?}", e).into())
            })
            .and_then(|slice| {
                u16::from_be_bytes(slice)
                    .try_into()
                    .map_err(|_e| ConvertError::ZeroPktId)
            })?;

        Ok((id, 2))
    }
}

impl ToMqttBytes for PktId {
    fn convert_to_mqtt(&self) -> Vec<u8> {
        let mut buf: Vec<u8> = Vec::with_capacity(2);
        buf.extend_from_slice(&self.get().to_be_bytes());
        buf
    }
}

mod connack;
mod connect;
mod disconnect;
mod pingreq;
mod pingresp;
mod properties;
mod puback;
pub(crate) mod publish;
mod qos;
mod suback;
mod subscribe;
mod unsuback;
mod unsubscribe;
