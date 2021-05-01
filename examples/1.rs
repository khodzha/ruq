use futures::StreamExt;
use log::warn;
use ruq::{Client, QoS};
use tokio::time::Duration;

const TOPIC: &str = "topic/test";

#[tokio::main]
async fn main() {
    pretty_env_logger::init_timed();

    let handle = tokio::runtime::Handle::current();
    let handle_ = handle.clone();

    let (mut client, evloop, mut receiver) = Client::new("0.0.0.0:1883");

    let jh1 = std::thread::spawn(move || {
        warn!("spawning future");
        handle.block_on(evloop)
    });

    client.subscribe(TOPIC.into(), ruq::QoS::AtMostOnce);

    let mut client_ = client.clone();

    let jh2 = std::thread::spawn(move || {
        handle_.block_on(async move {
            let mut disc = false;
            while let Some(x) = receiver.next().await {
                warn!("RECEIVER: {:?}", x);

                if let ruq::Notification::Message(_) = x {
                    disc = true;
                    client_.unsubscribe(TOPIC.into());
                }

                if let ruq::Notification::UnsubAck(_) = x {
                    if disc {
                        client_.disconnect();
                    }
                }
            }
        })
    });

    tokio::time::sleep(Duration::from_secs(2)).await;

    client.publish(
        "topic/test".into(),
        "hello from 1.rs".into(),
        QoS::AtMostOnce,
    );

    tokio::time::sleep(Duration::from_secs(2)).await;

    println!("Hello world");

    jh1.join().expect("JH1 erred");
    jh2.join().expect("JH1 erred");

    println!("Exiting");
}
