use std::convert::{TryFrom, TryInto};

use super::properties::Property;
use super::QoS;
use super::{ConvertError, FromMqttBytes, ToMqttBytes, VBI};

#[derive(Copy, Clone, Debug)]
pub struct PublishFlags {
    pub dup: bool,
    pub qos: QoS,
    pub retain: bool,
}

impl TryFrom<u8> for PublishFlags {
    type Error = ConvertError;

    fn try_from(byte: u8) -> Result<Self, Self::Error> {
        Ok(PublishFlags {
            dup: (byte & 0b1000) == 0b1000,
            qos: ((byte >> 1) & 0b11).try_into()?,
            retain: byte & 0b1 == 1,
        })
    }
}

impl TryFrom<&u8> for PublishFlags {
    type Error = ConvertError;

    fn try_from(byte: &u8) -> Result<Self, Self::Error> {
        TryFrom::<u8>::try_from(*byte)
    }
}

impl From<PublishFlags> for u8 {
    fn from(flags: PublishFlags) -> u8 {
        let mut b = 0;
        b += if flags.dup { 0b1000 } else { 0 };
        b += match flags.qos {
            QoS::AtMostOnce => 0,
            QoS::AtLeastOnce => 0b010,
            QoS::ExactlyOnce => 0b100,
        };
        b += if flags.retain { 1 } else { 0 };

        b
    }
}

#[derive(Clone, Debug)]
pub struct Publish {
    topic: String,
    payload: Payload,
    properties: Vec<Property>,
    flags: PublishFlags,
    pktid: Option<u16>,
}

#[derive(Clone, Debug)]
pub(crate) enum Payload {
    String(String),
    Bytes(Vec<u8>),
}

impl From<&str> for Payload {
    fn from(s: &str) -> Self {
        Payload::String(s.to_owned())
    }
}

impl From<&[u8]> for Payload {
    fn from(s: &[u8]) -> Self {
        Payload::Bytes(s.to_owned())
    }
}

impl ToMqttBytes for Payload {
    fn convert_to_mqtt(&self) -> Vec<u8> {
        match self {
            Payload::Bytes(v) => v.clone(),
            Payload::String(s) => s.as_bytes().to_owned(),
        }
    }
}

impl Publish {
    pub(crate) fn at_most_once(
        topic: String,
        payload: Payload,
        retain: bool,
        properties: Vec<Property>,
    ) -> Self {
        Self::new(topic, payload, retain, properties, None, QoS::AtMostOnce)
    }

    pub(crate) fn at_least_once(
        topic: String,
        payload: Payload,
        retain: bool,
        properties: Vec<Property>,
        pktid: u16,
    ) -> Self {
        Self::new(
            topic,
            payload,
            retain,
            properties,
            Some(pktid),
            QoS::AtLeastOnce,
        )
    }

    pub(crate) fn exactly_once(
        topic: String,
        payload: Payload,
        retain: bool,
        properties: Vec<Property>,
        pktid: u16,
    ) -> Self {
        Self::new(
            topic,
            payload,
            retain,
            properties,
            Some(pktid),
            QoS::ExactlyOnce,
        )
    }

    fn new(
        topic: String,
        payload: Payload,
        retain: bool,
        properties: Vec<Property>,
        pktid: Option<u16>,
        qos: QoS,
    ) -> Self {
        let flags = PublishFlags {
            qos,
            retain,
            dup: false,
        };

        Self {
            payload,
            topic,
            properties,
            flags,
            pktid,
        }
    }
}

impl ToMqttBytes for Publish {
    fn convert_to_mqtt(&self) -> Vec<u8> {
        let flags: u8 = self.flags.into();

        let first_byte: u8 = 0b0011_0000 + flags;
        let mut header: Vec<u8> = vec![first_byte];
        let mut buf = vec![];

        buf.extend_from_slice(&self.topic.convert_to_mqtt());
        if self.flags.qos > QoS::AtMostOnce {
            buf.extend_from_slice(&self.pktid.unwrap().to_be_bytes())
        }

        // properties
        buf.extend_from_slice(&self.properties.as_slice().convert_to_mqtt());

        buf.extend_from_slice(&self.payload.convert_to_mqtt());

        let len_vbi = VBI(buf.len() as u32).convert_to_mqtt();
        header.extend_from_slice(&len_vbi);
        header.extend_from_slice(&buf);

        header
    }
}

impl FromMqttBytes for Publish {
    fn convert_from_mqtt(bytes: &[u8]) -> Result<(Self, usize), ConvertError> {
        let mut bytes_read = 0;

        let flags: PublishFlags = bytes
            .get(0)
            .ok_or(ConvertError::NotEnoughBytes)?
            .try_into()?;
        bytes_read += 1;

        let (remaining_len, vbi_bytes_read) = VBI::convert_from_mqtt(&bytes[bytes_read..])?;
        bytes_read += vbi_bytes_read;

        let mut variable_header_len = 0;

        if bytes.len() < remaining_len.as_u32() as usize + vbi_bytes_read + 1 {
            return Err(ConvertError::NotEnoughBytes);
        }

        let (topic, topic_bytes_read) = String::convert_from_mqtt(&bytes[bytes_read..])?;
        bytes_read += topic_bytes_read;
        variable_header_len += topic_bytes_read;

        let pktid = if flags.qos > QoS::AtMostOnce {
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
            variable_header_len += 2;
            Some(pktid)
        } else {
            None
        };

        // TODO: read packet identifier if qos > 0
        // https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html#_Toc358219870

        let (properties, props_bytes_read) =
            Vec::<Property>::convert_from_mqtt(&bytes[bytes_read..])?;
        bytes_read += props_bytes_read;
        variable_header_len += props_bytes_read;

        let payload_len = remaining_len.as_u32() as usize - variable_header_len;
        let payload: Payload =
            match std::str::from_utf8(&bytes[bytes_read..bytes_read + payload_len]) {
                Ok(s) => s.into(),
                Err(_e) => (&bytes[bytes_read..bytes_read + payload_len]).into(),
            };
        bytes_read += payload_len;

        let publish = Self {
            flags,
            topic,
            payload,
            properties,
            pktid,
        };
        Ok((publish, bytes_read))
    }
}
