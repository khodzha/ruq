use super::ToMqttBytes;

#[derive(Debug)]
pub struct PingReq;

impl PingReq {
    pub fn new() -> Self {
        Self
    }
}

impl ToMqttBytes for PingReq {
    fn convert_to_mqtt(&self) -> Vec<u8> {
        vec![0b11000000, 0]
    }
}
