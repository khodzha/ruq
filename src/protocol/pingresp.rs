use super::{ConvertError, FromMqttBytes};

#[derive(Debug)]
pub struct PingResp;

impl FromMqttBytes for PingResp {
    fn convert_from_mqtt(bytes: &[u8]) -> Result<(Self, usize), ConvertError> {
        if bytes.len() < 2 {
            return Err(ConvertError::NotEnoughBytes);
        }

        Ok((PingResp, 2))
    }
}
