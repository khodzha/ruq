use super::{PktId, QoS, ToMqttBytes, VBI};

#[derive(Debug)]
pub struct Subscribe {
    topic: String,
    pktid: PktId,
    qos: QoS,
    no_local: bool,
    retain_as_published: bool,
}

impl Subscribe {
    pub fn new(topic: &str, pktid: PktId, qos: QoS) -> Self {
        Self {
            topic: topic.to_owned(),
            pktid,
            qos,
            // TODO: expose these fields, maybe through SubscribeBuilder not to blow new()?
            no_local: false,
            retain_as_published: false,
            // TODO:
            // 3.8.3.1 Subscription Options
            // retain_handling
        }
    }

    fn options(&self) -> u8 {
        let qos: u8 = self.qos.into();
        qos + (if self.no_local { 0b100 } else { 0 })
            + (if self.retain_as_published { 0b1000 } else { 0 })
    }
}

impl ToMqttBytes for Subscribe {
    fn convert_to_mqtt(&self) -> Vec<u8> {
        let mut header: Vec<u8> = vec![0b1000_0010];
        let mut buf = vec![];

        buf.extend_from_slice(&self.pktid.convert_to_mqtt());
        // TODO: sub properties
        // empty properties
        buf.push(0);

        // payload
        // topic
        buf.extend_from_slice(&self.topic.convert_to_mqtt());
        // 3.8.3.1 Subscription Options
        buf.push(self.options());

        let len_vbi = VBI(buf.len() as u32).convert_to_mqtt();
        header.extend_from_slice(&len_vbi);
        header.extend_from_slice(&buf);

        header
    }
}
