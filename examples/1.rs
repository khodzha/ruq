use futures::StreamExt;
use log::warn;
use ruq::{Client, Property, PublishBuilder, QoS};
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

    client.subscribe(TOPIC, ruq::QoS::AtMostOnce);

    let mut client_ = client.clone();

    let jh2 = std::thread::spawn(move || {
        handle_.block_on(async move {
            let mut disc = false;
            while let Some(x) = receiver.next().await {
                warn!("RECEIVER: {:?}", x);

                if let ruq::Notification::Message(_) = x {
                    disc = true;
                    client_.unsubscribe(TOPIC);
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

    let msg = PublishBuilder::from_str("topic/test2", "{\"message\": \"hello from 1.rs\"}")
        .qos(QoS::AtLeastOnce)
        .with_property(Property::UserProperty("type".into(), "event".into()))
        .with_property(Property::UserProperty("method".into(), "foobar2".into()))
        .with_property(Property::UserProperty(
            "local_initial_timediff".into(),
            "0".into(),
        ));

    for _ in 0..300 {
        client.publish(msg.clone());
    }
    // client.publish(msg.clone());

    tokio::time::sleep(Duration::from_secs(2)).await;

    println!("Hello world");

    for _ in 0..5 {
        client.publish(msg.clone());
    }

    jh1.join().expect("JH1 erred");
    jh2.join().expect("JH1 erred");

    println!("Exiting");
}
