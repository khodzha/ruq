use std::io::{Error as IOError, ErrorKind, Result as IOResult};
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use std::collections::VecDeque;

use anyhow::{anyhow, Error as AnyError, Result as AnyResult};
use futures::channel::mpsc::{Receiver, Sender};
use futures::{Future, SinkExt, StreamExt};
use pin_project::pin_project;
use protocol::publish::Payload;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::TcpStream;
use tokio::net::ToSocketAddrs;
use tokio::time::{Duration, Instant};
use tokio_util::codec::{FramedRead, FramedWrite};

use log::{error, info, warn};

use crate::protocol::{DisconnectReason, PktId, Publish};

pub use crate::client::{Client, Notification, PublishBuilder};
pub use crate::protocol::Property;
pub use crate::protocol::QoS;

pub trait TCPConnectFuture: Future<Output = IOResult<TcpStream>> + Send {}

impl<T> TCPConnectFuture for T where T: Future<Output = IOResult<TcpStream>> + Send {}

struct MQTTFuture {
    f: Pin<Box<dyn Future<Output = IOResult<()>> + Send>>,
}

impl Future for MQTTFuture {
    type Output = IOResult<()>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.as_mut().f).poll(cx)
    }
}

#[pin_project(project = ELProj)]
enum EventLoopState<F: TCPConnectFuture> {
    NotConnected {
        #[pin]
        f: F,
    },
    MqttConnected {
        #[pin]
        f: MQTTFuture,
    },
}

#[pin_project]
pub struct EventLoop<A, F>
where
    A: ToSocketAddrs + Send,
    F: TCPConnectFuture,
{
    #[pin]
    state: EventLoopState<F>,
    commands_rx: Option<Receiver<Command>>,
    notifications_tx: Sender<Notification>,
    address: A,
}

impl<A, F> Future for EventLoop<A, F>
where
    A: ToSocketAddrs + Send,
    F: TCPConnectFuture,
{
    type Output = AnyResult<()>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut me = self.project();
        loop {
            let next = {
                match me.state.as_mut().project() {
                    ELProj::NotConnected { mut f } => match f.as_mut().poll(cx) {
                        Poll::Ready(Err(e)) => {
                            return Poll::Ready(Err(anyhow!(
                                "Failed to connect, reason = {:?}",
                                e
                            )));
                        }
                        Poll::Pending => {
                            return Poll::Pending;
                        }
                        Poll::Ready(Ok(tcp_stream)) => {
                            let (read_half, write_half) = tcp_stream.into_split();
                            let mqtt_read =
                                FramedRead::new(read_half, mqtt_codec::MqttCodec::new());
                            let mqtt_write =
                                FramedWrite::new(write_half, mqtt_codec::MqttCodec::new());

                            let f = Box::pin(poll(
                                mqtt_read,
                                mqtt_write,
                                me.commands_rx.take().unwrap(),
                                me.notifications_tx.clone(),
                            ));

                            EventLoopState::MqttConnected {
                                f: MQTTFuture { f: f },
                            }
                        }
                    },
                    ELProj::MqttConnected { f } => {
                        return f.poll(cx).map_err(|e| e.into());
                    }
                }
            };

            me.state.set(next);
        }
    }
}

async fn poll(
    mut mqtt_read: FramedRead<OwnedReadHalf, mqtt_codec::MqttCodec>,
    mut mqtt_write: FramedWrite<OwnedWriteHalf, mqtt_codec::MqttCodec>,
    mut commands_rx: Receiver<Command>,
    mut sender: Sender<Notification>,
) -> IOResult<()> {
    let mut pkt = protocol::Connect::new("whatever.devops.svc.example.org");
    pkt.keep_alive(8);

    mqtt_write.send(protocol::Packet::Connect(pkt)).await?;

    let pkt = mqtt_read.next().await;
    // TODO: pkt must be a connack with ConnackReason::Success
    match sender
        .send(Notification::ConnAck(format!("{:?}", pkt)))
        .await
    {
        Ok(()) => {}
        Err(e) => {
            // TODO: if we failed to send notification to client code, what should we do here and below?
            // probably send disconnect without acking any incoming messages?
        }
    }

    let mut ping = tokio::time::interval_at(
        Instant::now() + Duration::from_secs(8),
        Duration::from_secs(8),
    );
    let mut pingresp_received = None;

    let mut pktids = pktids::PktIds::new();
    // todo: on conversion from future<output = connection_result> to stream<item = connection_result>
    // we should save this queue on disconnect and read from it first if user establishes new connection
    let mut queue: VecDeque<(PktId, Box<Publish>)> = VecDeque::new();

    loop {
        tokio::select! {
            _ = ping.tick() => {
                if pingresp_received == Some(false) {
                    return Err(IOError::new(ErrorKind::TimedOut, "PingResp timed out"))
                }
                let pkt = protocol::PingReq::new();
                mqtt_write.send(protocol::Packet::PingReq(pkt)).await?;
                pingresp_received = Some(false);
            }
            req = mqtt_read.next() => {
                info!("Incoming req: {:?}", req);
                match req {
                    Some(Ok(x)) => {
                        match x {
                            protocol::Packet::Disconnect(disconnect) => {
                                info!("Got disconnect packet from server: {:?}", disconnect);
                                return match disconnect.reason() {
                                    DisconnectReason::Normal => Ok(()),
                                    reason => {
                                        let e = format!("Disconnected due to: {:?}", reason);
                                        Err(IOError::new(ErrorKind::Other, e))
                                    },
                                }
                            },
                            protocol::Packet::PingResp(_) => {
                                pingresp_received = Some(true);
                            }
                            protocol::Packet::SubAck(m) => {
                                pktids.return_id(m.pktid());
                                sender.send(Notification::SubAck(format!("{:?}", m))).await;
                            }
                            protocol::Packet::UnsubAck(m) => {
                                pktids.return_id(m.pktid());
                                sender.send(Notification::UnsubAck(format!("{:?}", m))).await;
                            }
                            protocol::Packet::PubAck(m) => {
                                if let Some(idx) = queue.iter().position(|(pktid, _)| *pktid == m.pktid()) {
                                    queue.remove(idx);
                                } else {
                                    warn!("Failed to find message to ack in the queue");
                                }

                                pktids.return_id(m.pktid());
                            }
                            protocol::Packet::Publish(m) => {
                                match m.qos() {
                                    QoS::AtMostOnce => {},
                                    QoS::AtLeastOnce => {
                                        let pkt = protocol::Puback::new(m.pktid().unwrap());
                                        mqtt_write.send(protocol::Packet::PubAck(pkt)).await?;
                                    }
                                    QoS::ExactlyOnce => {
                                        todo!("Send pubrec");
                                    }
                                }

                                sender.send(Notification::Message(format!("{:?}", m))).await;
                            }
                            m => {
                                sender.send(Notification::Message(format!("{:?}", m))).await;
                            }
                        }
                    }
                    Some(Err(e)) => {
                        error!("Incoming req error: {:?}", e);
                        return Err(e);
                    }
                    None => {
                        // stream done
                        return Ok(());
                    }
                }
            },
            cmd = commands_rx.next() => {
                match cmd {
                    Some(x) => match x {
                        Command::Publish { topic, payload, properties, qos, retain } => {
                            let pkt = match qos {
                                QoS::AtMostOnce => {
                                    protocol::Publish::at_most_once(topic, payload, retain, properties)
                                }
                                QoS::AtLeastOnce => {
                                    match pktids.next_id() {
                                        Some(pktid) => {
                                            let pkt = protocol::Publish::at_least_once(topic, payload, retain, properties, pktid);
                                            queue.push_back((pktid, Box::new(pkt.clone())));
                                            pkt
                                        }
                                        None => todo!(),
                                    }
                                }
                                QoS::ExactlyOnce => {
                                    match pktids.next_id() {
                                        Some(pktid) => protocol::Publish::exactly_once(topic, payload, retain, properties, pktid),
                                        None => todo!(),
                                    }
                                }
                            };
                            mqtt_write.send(protocol::Packet::Publish(pkt)).await?;
                        }
                        Command::Subscribe(topic, qos) => {
                            let pkt = match pktids.next_id() {
                                Some(pktid) => protocol::Subscribe::new(&topic, pktid, qos),
                                None => todo!(),
                            };
                            mqtt_write.send(protocol::Packet::Subscribe(pkt)).await?;
                        }
                        Command::Unsubscribe(topics) => {
                            let pkt = match pktids.next_id() {
                                Some(pktid) => protocol::Unsubscribe::new(topics, pktid),
                                None => todo!(),
                            };
                            mqtt_write.send(protocol::Packet::Unsubscribe(pkt)).await?;
                        }
                        Command::Disconnect => {
                            let pkt = protocol::Disconnect::new();
                            mqtt_write.send(protocol::Packet::Disconnect(pkt)).await?;
                            return Ok(());
                        }
                    }
                    None => {}
                }
            }
        }
    }
}

pub(crate) enum Command {
    Publish {
        topic: String,
        payload: Payload,
        properties: Vec<Property>,
        qos: QoS,
        retain: bool,
    },
    Subscribe(String, QoS),
    Unsubscribe(Vec<String>),
    Disconnect,
}

mod client;
mod mqtt_codec;
mod pktids;
mod protocol;

#[cfg(test)]
mod tests {
    use crate::protocol::FromMqttBytes;

    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }

    #[test]
    fn parse_invalid_string() {
        let bytes = [6, 0, b'f', b'o', b'o', b'b', b'a', b'r'];
        assert!(String::convert_from_mqtt(&bytes).is_err());
    }
}
