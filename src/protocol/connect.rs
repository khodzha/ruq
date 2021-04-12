use super::ToMqttBytes;
use super::VBI;

#[derive(Debug)]
pub struct Connect {
    client_id: String,
    keep_alive: u16,
}

impl Connect {
    pub fn new(client_id: &str) -> Self {
        Self {
            client_id: client_id.to_owned(),
            keep_alive: 0,
        }
    }

    pub fn keep_alive(&mut self, keep_alive: u16) {
        self.keep_alive = keep_alive;
    }
}

impl ToMqttBytes for Connect {
    fn convert_to_mqtt(&self) -> Vec<u8> {
        let mut header: Vec<u8> = vec![0b00010000];
        let mut buf = vec![];
        // protocol
        buf.extend_from_slice(&"MQTT".convert_to_mqtt());
        // version
        buf.push(5);
        // connect flags
        // clean_start = 0b0000_0010
        // no clean_start = 0b0000_0000
        buf.push(0b0000_0010);

        // keep alive
        buf.extend_from_slice(&self.keep_alive.to_be_bytes());

        // connect props
        let slice = VBI(0).convert_to_mqtt();
        buf.extend_from_slice(&slice);

        // client id
        buf.extend_from_slice(&self.client_id.convert_to_mqtt());

        let len_vbi = VBI(buf.len() as u32).convert_to_mqtt();
        header.extend_from_slice(&len_vbi);
        header.extend_from_slice(&buf);

        header
    }
}
