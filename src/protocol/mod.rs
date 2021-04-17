use std::convert::{TryFrom, TryInto};

pub use connack::Connack;
pub use connect::Connect;
pub use pingreq::PingReq;
pub use pingresp::PingResp;
pub use properties::Property;
pub use puback::Puback;
pub use publish::Publish;
pub use publish::QoS;
pub use subscribe::Subscribe;
pub use suback::Suback;
pub use unsubscribe::Unsubscribe;
pub use unsuback::Unsuback;

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

    match pkt_type {
        PacketType::Publish => match Publish::convert_from_mqtt(&data) {
            Ok((pkt, _bytes_read)) => return Ok(Some(Packet::Publish(pkt))),
            Err(ConvertError::NotEnoughBytes) => {
                info!("Received publish but not enough bytes?");
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("parse_pkt expects that full pkt is present"),
                ));
            }
            Err(e) => {
                error!(
                    "Something went wrong with parsing publish pkt, err = {:?}",
                    e
                );
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!(
                        "Something went wrong with parsing publish pkt, err = {:?}",
                        e
                    ),
                ));
            }
        },
        PacketType::Connack => match Connack::convert_from_mqtt(&data) {
            Ok((pkt, _bytes_read)) => return Ok(Some(Packet::Connack(pkt))),
            Err(ConvertError::NotEnoughBytes) => {
                info!("Received publish but not enough bytes?");
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("parse_pkt expects that full pkt is present"),
                ));
            }
            Err(e) => {
                error!(
                    "Something went wrong with parsing connack pkt, err = {:?}",
                    e
                );
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!(
                        "Something went wrong with parsing connack pkt, err = {:?}",
                        e
                    ),
                ));
            }
        },
        PacketType::PingResponse => match PingResp::convert_from_mqtt(&data) {
            Ok((pkt, _bytes_read)) => return Ok(Some(Packet::PingResp(pkt))),
            Err(ConvertError::NotEnoughBytes) => {
                info!("Received publish but not enough bytes?");
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("parse_pkt expects that full pkt is present"),
                ));
            }
            Err(e) => {
                error!(
                    "Something went wrong with parsing connack pkt, err = {:?}",
                    e
                );
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!(
                        "Something went wrong with parsing connack pkt, err = {:?}",
                        e
                    ),
                ));
            }
        },
        PacketType::SubscribeAck => match Suback::convert_from_mqtt(&data) {
            Ok((pkt, _bytes_read)) => return Ok(Some(Packet::SubAck(pkt))),
            Err(ConvertError::NotEnoughBytes) => {
                info!("Received publish but not enough bytes?");
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("parse_pkt expects that full pkt is present"),
                ));
            }
            Err(e) => {
                error!(
                    "Something went wrong with parsing connack pkt, err = {:?}",
                    e
                );
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!(
                        "Something went wrong with parsing connack pkt, err = {:?}",
                        e
                    ),
                ));
            }
        },
        PacketType::SubscribeAck => match Suback::convert_from_mqtt(&data) {
            Ok((pkt, _bytes_read)) => return Ok(Some(Packet::SubAck(pkt))),
            Err(ConvertError::NotEnoughBytes) => {
                info!("Received publish but not enough bytes?");
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("parse_pkt expects that full pkt is present"),
                ));
            }
            Err(e) => {
                error!(
                    "Something went wrong with parsing connack pkt, err = {:?}",
                    e
                );
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!(
                        "Something went wrong with parsing connack pkt, err = {:?}",
                        e
                    ),
                ));
            }
        },
        PacketType::UnsubscribeAck => match Unsuback::convert_from_mqtt(&data) {
            Ok((pkt, _bytes_read)) => return Ok(Some(Packet::UnsubAck(pkt))),
            Err(ConvertError::NotEnoughBytes) => {
                info!("Received publish but not enough bytes?");
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("parse_pkt expects that full pkt is present"),
                ));
            }
            Err(e) => {
                error!(
                    "Something went wrong with parsing connack pkt, err = {:?}",
                    e
                );
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!(
                        "Something went wrong with parsing connack pkt, err = {:?}",
                        e
                    ),
                ));
            }
        },
        ty => {
            info!("Received msg type = {:?}", ty);
            unimplemented!("This is not impl");
        }
    }
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
    /* TODO:
    PubReceived,
    PubRelease,
    PubComplete,
    PingResponse,
    Disconnect,
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
            _ => unimplemented!(),
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

#[derive(Debug)]
pub struct VBI(u32);

impl VBI {
    pub fn as_u32(&self) -> u32 {
        self.0
    }
}

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
        if len > 0 {
            let s = std::str::from_utf8(&bytes[2..(2 + len)])
                .map_err(|e| format!("Failed to parse bytes as utf8, reason = {:?}", e))?;

            Ok((s.into(), len + 2))
        } else {
            Ok(("".into(), 2))
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

mod connack;
mod connect;
mod pingreq;
mod pingresp;
mod properties;
mod puback;
mod publish;
mod subscribe;
mod suback;
mod unsubscribe;
mod unsuback;
