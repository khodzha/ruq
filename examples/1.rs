use ruq::Client;

use futures::StreamExt;

#[tokio::main]
async fn main() {
    let handle = tokio::runtime::Handle::current();

    let (mut client, evloop, mut receiver) = Client::new("0.0.0.0:1883");

    std::thread::spawn(move || {
        eprintln!("spawning future");
        handle.block_on(evloop)
    });

    client.subscribe("topic/test".into(), ruq::QoS::AtMostOnce);

    while let Some(x) = receiver.next().await {
        eprintln!("RECEIVER: {:?}", x)
    }
    println!("Hello world");
}
