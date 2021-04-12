use bytes::{Buf, BytesMut};
use tokio_util::codec::{Decoder, Encoder};

use super::protocol;
use super::protocol::{ConvertError, FromMqttBytes, PacketType, Publish, ToMqttBytes, Packet};

pub struct MqttCodec;

impl MqttCodec {
    pub fn new() -> Self {
        Self
    }
}

// Packet size is 1 byte for fixed header + len of VBI (at most 4 bytes) + value of VBI for remaining len
// And the maximum value of VBI is 268_435_455
// https://docs.oasis-open.org/mqtt/mqtt/v5.0/os/mqtt-v5.0-os.html#_Toc3901011
// So maximum packet len is 268_435_455 + 4 + 1
const MAX: usize = 268_435_460;

impl Decoder for MqttCodec {
    type Item = protocol::Packet;
    type Error = std::io::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        // we need 1 byte for header and at least 1 byte to start parsing remaining len VBI
        if src.len() < 2 {
            return Ok(None);
        }

        protocol::parse_fixed_header(src[0]).map_err(|e| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("Byte {:?} is not a proper packet header.", src[0]),
            )
        })?;

        match protocol::VBI::convert_from_mqtt(&src[1..]) {
            Ok((len, vbi_len)) => {
                let total_pkt_len = 1 + vbi_len + len.as_u32() as usize;
                if src.len() < total_pkt_len {
                    return Ok(None);
                } else {
                    // Use advance to modify src such that it no longer contains
                    // this frame.
                    let data = src[0..total_pkt_len].to_vec();
                    src.advance(total_pkt_len);

                    // Convert the data to a string, or fail if it is not valid utf-8.
                    protocol::parse_pkt(&data)
                }
            }
            Err(protocol::ConvertError::NotEnoughBytes) => {
                return Ok(None);
            }
            Err(e) => {
                return Err(e.into());
            }
        }
    }
}

impl Encoder<Packet> for MqttCodec {
    type Error = std::io::Error;

    fn encode(&mut self, item: Packet, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let data = item.convert_to_mqtt();
        dst.extend_from_slice(&data);
        Ok(())
    }
}
