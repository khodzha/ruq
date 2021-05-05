use anyhow::{Context as AnyContext, Error};
use futures::channel::mpsc::{channel, Receiver, Sender};
use tokio::net::TcpStream;
use tokio::net::ToSocketAddrs;

use crate::protocol::Property;
use crate::{Payload, QoS};

use super::*;

#[derive(Clone)]
pub struct Client {
    tx: Sender<Command>,
}

#[derive(Debug)]
pub enum Notification {
    ConnAck(String),
    SubAck(String),
    Message(String),
    UnsubAck(String),
}

impl Client {
    pub fn new<A: ToOwned>(
        address: A,
    ) -> (
        Self,
        EventLoop<A::Owned, impl TCPConnectFuture>,
        Receiver<Notification>,
    )
    where
        A: ToOwned,
        A::Owned: ToSocketAddrs + Clone + Send + 'static,
    {
        let (tx, rx) = channel(100);
        let (notif_tx, notif_rx) = channel(100);

        let address = address.to_owned();
        let address_ = address.clone();
        let p = async move { TcpStream::connect(address_).await };

        let evloop = EventLoop {
            address,
            commands_rx: Some(rx),
            notifications_tx: notif_tx,
            state: EventLoopState::NotConnected { f: p },
        };

        (Self { tx }, evloop, notif_rx)
    }

    pub fn connect(&mut self) -> Result<(), Error> {
        /*self.tx
        .try_send(Command::Connect)
        .with_context(|| "Connection failed")*/
        Ok(())
    }

    pub fn subscribe(&mut self, topic: &str, qos: QoS) -> Result<(), Error> {
        self.tx
            .try_send(Command::Subscribe(topic.to_owned(), qos))
            .with_context(|| "Subscribe chan send failed")
    }

    pub fn publish(&mut self, publish_builder: PublishBuilder) -> Result<(), Error> {
        self.tx
            .try_send(publish_builder.build())
            .with_context(|| "Publish chan send failed")
    }

    pub fn unsubscribe(&mut self, topic: &str) -> Result<(), Error> {
        self.tx
            .try_send(Command::Unsubscribe(vec![topic.to_owned()]))
            .with_context(|| "Unsubscribe chan send failed")
    }

    pub fn disconnect(&mut self) -> Result<(), Error> {
        self.tx
            .try_send(Command::Disconnect)
            .with_context(|| "Disconnect chan send failed")
    }
}

#[derive(Clone)]
pub struct PublishBuilder {
    topic: String,
    payload: Payload,
    properties: Vec<Property>,
    qos: Option<QoS>,
    retain: Option<bool>,
}

impl PublishBuilder {
    pub fn from_bytes(topic: &str, payload: &[u8]) -> Self {
        Self {
            topic: topic.to_owned(),
            payload: Payload::Bytes(payload.to_vec()),
            properties: vec![Property::PayloadFormatId(0x00)],
            qos: None,
            retain: None,
        }
    }

    pub fn from_str(topic: &str, payload: &str) -> Self {
        Self {
            topic: topic.to_owned(),
            payload: Payload::String(payload.to_string()),
            properties: vec![Property::PayloadFormatId(0x01)],
            qos: None,
            retain: None,
        }
    }

    pub fn qos(mut self, qos: QoS) -> Self {
        self.qos = Some(qos);
        self
    }

    pub fn with_property(mut self, prop: Property) -> Self {
        self.properties.push(prop);
        self
    }

    fn build(self) -> Command {
        Command::Publish {
            topic: self.topic,
            payload: self.payload,
            properties: self.properties,
            qos: self.qos.unwrap_or(QoS::AtMostOnce),
            retain: self.retain.unwrap_or(false),
        }
    }
}
