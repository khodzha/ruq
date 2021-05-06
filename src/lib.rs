use std::io::Result as IOResult;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;

use anyhow::{anyhow, Result as AnyResult, Error as AnyError};
use futures::channel::mpsc::{Receiver, Sender};
use futures::{Future, SinkExt, StreamExt};
use pin_project::pin_project;
use protocol::publish::Payload;
use tokio::net::TcpStream;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use tokio::net::ToSocketAddrs;
use tokio::time::{Duration, Instant};
use tokio_util::codec::{FramedRead, FramedWrite};

use log::info;

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
                            let mqtt_read = FramedRead::new(read_half, mqtt_codec::MqttCodec::new());
                            let mqtt_write = FramedWrite::new(write_half, mqtt_codec::MqttCodec::new());

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
                    /*ELProj::Connected { mqtt_stream, state } => {
                        match state {
                            MqttConnectState::Connect(connect) => match connect.as_mut().poll(cx) {
                                Poll::Pending => {
                                    return Poll::Pending;
                                }
                                Poll::Ready(Err(e)) => {
                                    return Poll::Ready(Err(e.into()));
                                }
                                Poll::Ready(Ok(())) => {
                                    let mqtt_stream_ = mqtt_stream.clone();
                                    let f = Box::pin(async move {
                                        mqtt_stream_.borrow_mut().next().await
                                    });
                                    EventLoopState::Connected { mqtt_stream: mqtt_stream.clone(), state: MqttConnectState::Connack(f) }
                                }
                            },
                            MqttConnectState::Connack(connack) => match connack.as_mut().poll(cx) {
                                Poll::Pending => {
                                    return Poll::Pending;
                                }
                                Poll::Ready(None) => {
                                    return Poll::Ready(Err(anyhow!("Stream done")));
                                }
                                Poll::Ready(Some(Err(e))) => {
                                    return Poll::Ready(Err(e.into()));
                                },
                                Poll::Ready(Some(Ok(pkt))) => {
                                    eprintln!("Received connack: {:?}", pkt);
                                    let ping = sleep(Duration::from_secs(8));
                                    EventLoopState::MqttConnected {
                                        mqtt_stream: mqtt_stream.clone(),
                                        next_ping: ping,
                                    }
                                }
                            }
                        }
                    }*/
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
    sender
        .send(Notification::ConnAck(format!("{:?}", pkt)))
        .await;

    let mut ping = tokio::time::interval_at(
        Instant::now() + Duration::from_secs(8),
        Duration::from_secs(8),
    );
    let mut pingresp_received = None;

    let mut pktids = pktids::PktIds::new();

    loop {
        tokio::select! {
            _ = ping.tick() => {
                if pingresp_received == Some(false) {
                    return Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "PingResp timed out"))
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
                                pktids.return_id(m.pktid());
                            }
                            m => {
                                sender.send(Notification::Message(format!("{:?}", m))).await;
                            }
                        }
                    }
                    _ => {}
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
                                        Some(pktid) => protocol::Publish::at_least_once(topic, payload, retain, properties, pktid),
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
                            mqtt_write.send(protocol::Packet::Publish(pkt)).await;
                        }
                        Command::Subscribe(topic, qos) => {
                            let pkt = match pktids.next_id() {
                                Some(pktid) => protocol::Subscribe::new(&topic, pktid, qos),
                                None => todo!(),
                            };
                            mqtt_write.send(protocol::Packet::Subscribe(pkt)).await;
                        }
                        Command::Unsubscribe(topics) => {
                            let pkt = match pktids.next_id() {
                                Some(pktid) => protocol::Unsubscribe::new(topics, pktid),
                                None => todo!(),
                            };
                            mqtt_write.send(protocol::Packet::Unsubscribe(pkt)).await;
                        }
                        Command::Disconnect => {
                            let pkt = protocol::Disconnect::new();
                            mqtt_write.send(protocol::Packet::Disconnect(pkt)).await;
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
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
