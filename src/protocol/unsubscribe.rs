use super::{PktId, ToMqttBytes, VBI};

#[derive(Debug)]
pub struct Unsubscribe {
    topics: Vec<String>,
    pktid: PktId,
}

impl Unsubscribe {
    pub fn new(topics: Vec<String>, pktid: PktId) -> Self {
        Self { topics, pktid }
    }
}

impl ToMqttBytes for Unsubscribe {
    fn convert_to_mqtt(&self) -> Vec<u8> {
        let mut header: Vec<u8> = vec![0b1010_0010];
        let mut buf = vec![];

        buf.extend_from_slice(&self.pktid.convert_to_mqtt());
        // TODO: unsub properties
        // empty properties
        buf.push(0);

        // payload
        // topic
        for topic in &self.topics {
            buf.extend_from_slice(&topic.convert_to_mqtt());
        }

        // TODO: precalc len
        let len_vbi = VBI(buf.len() as u32).convert_to_mqtt();
        header.extend_from_slice(&len_vbi);
        header.extend_from_slice(&buf);

        header
    }
}
