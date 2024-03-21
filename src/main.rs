use async_std::{
    net::TcpListener, task::spawn,
    prelude::*,
    io::{Read, Write},
};
use futures::stream::StreamExt;

// rust proxy prototype
// async version
#[async_std::main]
async fn main() {
    println!("rs proxy.");

    // config hardcode
    let addr = "0.0.0.0:4040";

    // init listener
    let ln = TcpListener::bind(addr).await.unwrap();
    ln.incoming().for_each_concurrent(None, |tcp_stream| async move {
        let tcp_stream = tcp_stream.unwrap();
        spawn(handle_connection(tcp_stream));
    }).await;

    println!("server stop.")
}

async fn handle_connection(mut stream: impl Read + Write + Unpin) {
    // request read
    let mut buffer = [0; 1024];
    let n = match stream.read(&mut buffer).await {
        Ok(n) => n,
        Err(err) => {
            println!("stream read error: {:?}", err);
            return;
        }
    };
    let received = std::str::from_utf8(&buffer[0..n]).expect("valid utf8");
    println!("stream read: {:?}", received);

    println!("conn handled.")
}
