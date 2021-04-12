use std::pin::Pin;
use std::task::Context;
use std::task::Poll;
use std::sync::Arc;
use std::cell::RefCell;

use anyhow::{anyhow, Context as AnyContext, Error, Result as AResult};
use futures::channel::mpsc::{channel, Receiver, Sender};
use futures::{Future, SinkExt, StreamExt};
use pin_project::pin_project;
use tokio::io::AsyncWriteExt;
use tokio_util::codec::{Framed};
use tokio::net::TcpStream;
use tokio::net::ToSocketAddrs;

use crate::protocol::{ConvertError, FromMqttBytes, PacketType, Publish, ToMqttBytes, Packet};

pub struct Client {
    tx: Sender<Command>,
}

pub enum Notification {
    Connack,
}

enum MqttConnectState {
    Connect(Pin<Box<dyn Future<Output=std::io::Result<()>>>>),
    Connack(Pin<Box<dyn Future<Output=Option<std::io::Result<Packet>>>>>)
}

#[pin_project(project = ELProj)]
enum EventLoopState {
    NotConnected {
        f: Pin<Box<dyn Future<Output=std::io::Result<TcpStream>>>>
    },
    Connected {
        mqtt_stream: Arc<RefCell<Framed<TcpStream, mqtt_stream::MqttCodec>>>,
        state: MqttConnectState
    },
    MqttConnected {
        mqtt_stream: Arc<RefCell<Framed<TcpStream, mqtt_stream::MqttCodec>>>,
    },
    GracefulShutdown,
    AbruptDisconnect,
}

#[pin_project]
pub struct EventLoop<A>
where
    A: ToSocketAddrs
{
    #[pin]
    state: EventLoopState,
    commands_rx: Receiver<Command>,
    notifications_tx: Sender<Notification>,
    address: A,
}

impl<A> Future for EventLoop<A>
where
    A: ToSocketAddrs,
{
    type Output = Result<(), Error>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut me = self.project();
        loop {
            let next = {
                match me.state.as_mut().project() {
                    ELProj::NotConnected { f } => {
                        match f.as_mut().poll(cx) {
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
                                let mqtt_stream = Arc::new(RefCell::new(Framed::new(
                                    tcp_stream,
                                    mqtt_stream::MqttCodec::new(),
                                )));
                                let mut pkt = protocol::Connect::new("whatever.devops.svc.example.org");
                                pkt.keep_alive(8);

                                let mqtt_stream_ = mqtt_stream.clone();
                                let f = Box::pin(async move {
                                    mqtt_stream_.borrow_mut().send(protocol::Packet::Connect(pkt)).await.map(|_| ())
                                });

                                EventLoopState::Connected { mqtt_stream, state: MqttConnectState::Connect(f) }
                            }
                        }
                    }
                    ELProj::Connected { mqtt_stream, state } => {
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
                                    EventLoopState::MqttConnected { mqtt_stream: mqtt_stream.clone() }
                                }
                            }
                        }
                    }
                    ELProj::MqttConnected { .. } => {
                        return Poll::Pending;
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

pub enum Command {
    Connect,
    Publish,
    Subscribe,
    Unsubscribe,
    Disconnect,
}

impl Client {
    pub fn new<A: ToOwned>(address: A) -> (Self, EventLoop<A::Owned>, Receiver<Notification>)
    where
        A: ToOwned,
        A::Owned: ToSocketAddrs + Clone + 'static
    {
        let (tx, rx) = channel(100);
        let (notif_tx, notif_rx) = channel(100);

        let address = address.to_owned();
        let address_ = address.clone();
        let p = Box::pin(async move {
            TcpStream::connect(&address_).await
        });

        let evloop = EventLoop {
            address,
            commands_rx: rx,
            notifications_tx: notif_tx,
            state: EventLoopState::NotConnected { f: p },
        };

        (Self { tx }, evloop, notif_rx)
    }

    pub fn connect(&mut self) -> Result<(), Error> {
        self.tx
            .try_send(Command::Connect)
            .with_context(|| "Connection failed")
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
