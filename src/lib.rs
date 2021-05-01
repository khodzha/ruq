use std::io::Result as IOResult;
use std::pin::Pin;
use std::task::Context;
use std::task::Poll;

use anyhow::{anyhow, Context as AnyContext, Error};
use futures::channel::mpsc::{channel, Receiver, Sender};
use futures::{Future, SinkExt, StreamExt};
use pin_project::pin_project;
use tokio::net::TcpStream;
use tokio::net::ToSocketAddrs;
use tokio::time::{Duration, Instant};
use tokio_util::codec::Framed;

use log::info;

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
    GracefulShutdown,
    AbruptDisconnect,
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
    type Output = Result<(), Error>;

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
                            let mqtt_stream =
                                Framed::new(tcp_stream, mqtt_stream::MqttCodec::new());

                            let f = Box::pin(poll(
                                mqtt_stream,
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
                    ELProj::AbruptDisconnect => {
                        return Poll::Pending;
                    }
                    ELProj::GracefulShutdown => {
                        return Poll::Pending;
                    }
                }
            };

            me.state.set(next);
        }
    }
}

async fn poll(
    mut mqtt_stream: Framed<TcpStream, mqtt_stream::MqttCodec>,
    mut commands_rx: Receiver<Command>,
    mut sender: Sender<Notification>,
) -> IOResult<()> {
    let mut pkt = protocol::Connect::new("whatever.devops.svc.example.org");
    pkt.keep_alive(8);

    mqtt_stream.send(protocol::Packet::Connect(pkt)).await?;

    let pkt = mqtt_stream.next().await;
    sender
        .send(Notification::ConnAck(format!("{:?}", pkt)))
        .await;

    let mut ping = tokio::time::interval_at(
        Instant::now() + Duration::from_secs(8),
        Duration::from_secs(8),
    );
    let mut pingresp_received = None;

    let mut pktid: u16 = 1;

    loop {
        let ping_tick = ping.tick();
        tokio::pin!(ping_tick);
        let next_mqtt = mqtt_stream.next();
        tokio::pin!(next_mqtt);
        let next_cmd = commands_rx.next();
        tokio::pin!(next_cmd);

        tokio::select! {
            _ = &mut ping_tick => {
                if pingresp_received == Some(false) {
                    return Err(std::io::Error::new(std::io::ErrorKind::TimedOut, "PingResp timed out"))
                }
                let pkt = protocol::PingReq::new();
                mqtt_stream.send(protocol::Packet::PingReq(pkt)).await?;
                pingresp_received = Some(false);
            }
            req = &mut next_mqtt => {
                info!("Incoming req: {:?}", req);
                match req {
                    Some(Ok(x)) => {
                        match x {
                            protocol::Packet::PingResp(_) => {
                                pingresp_received = Some(true);
                            }
                            protocol::Packet::SubAck(m) => {
                                sender.send(Notification::SubAck(format!("{:?}", m))).await;
                            }
                            protocol::Packet::UnsubAck(m) => {
                                sender.send(Notification::UnsubAck(format!("{:?}", m))).await;
                            }
                            m => {
                                sender.send(Notification::Message(format!("{:?}", m))).await;
                            }
                        }
                    }
                    _ => {}
                }
            },
            cmd = &mut next_cmd => {
                match cmd {
                    Some(x) => match x {
                        Command::Publish(topic, payload, qos) => {
                            let pkt = match qos {
                                QoS::AtMostOnce => {
                                    protocol::Publish::new(&topic, &payload, qos, None)
                                }
                                _ => {
                                    let pkt = protocol::Publish::new(&topic, &payload, qos, Some(pktid));
                                    pktid = pktid.wrapping_add(1);
                                    pkt
                                }
                            };
                            mqtt_stream.send(protocol::Packet::Publish(pkt)).await;
                        }
                        Command::Subscribe(topic, qos) => {
                            let pkt = protocol::Subscribe::new(&topic, pktid, qos);
                            pktid = pktid.wrapping_add(1);
                            mqtt_stream.send(protocol::Packet::Subscribe(pkt)).await;
                        }
                        Command::Unsubscribe(topics) => {
                            let pkt = protocol::Unsubscribe::new(topics, pktid);
                            pktid = pktid.wrapping_add(1);
                            mqtt_stream.send(protocol::Packet::Unsubscribe(pkt)).await;
                        }
                        Command::Disconnect => {
                            let pkt = protocol::Disconnect::new();
                            mqtt_stream.send(protocol::Packet::Disconnect(pkt)).await;
                            return Ok(());
                        }
                    }
                    None => {}
                }
            }
        }
    }
}

pub enum Command {
    Publish(String, String, QoS),
    Subscribe(String, QoS),
    Unsubscribe(Vec<String>),
    Disconnect,
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

    pub fn subscribe(&mut self, topic: String, qos: QoS) -> Result<(), Error> {
        self.tx
            .try_send(Command::Subscribe(topic, qos))
            .with_context(|| "Subscribe chan send failed")
    }

    pub fn publish(&mut self, topic: String, payload: String, qos: QoS) -> Result<(), Error> {
        self.tx
            .try_send(Command::Publish(topic, payload, qos))
            .with_context(|| "Publish chan send failed")
    }

    pub fn unsubscribe(&mut self, topic: String) -> Result<(), Error> {
        self.tx
            .try_send(Command::Unsubscribe(vec![topic]))
            .with_context(|| "Unsubscribe chan send failed")
    }

    pub fn disconnect(&mut self) -> Result<(), Error> {
        self.tx
            .try_send(Command::Disconnect)
            .with_context(|| "Disconnect chan send failed")
    }
}

mod mqtt_stream;
mod protocol;

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
}
