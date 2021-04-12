use ruq::Client;

#[tokio::main]
async fn main() {
    let (client, evloop, receiver) = Client::new("0.0.0.0:1883");

    evloop.await;
    println!("Hello world");
}
