use async_std::{
    future,
    io::{Error, ErrorKind, Read, Write},
    net::{TcpListener, TcpStream},
    prelude::*,
    task::spawn,
};
use futures::stream::StreamExt;
use futures::try_join;
use std::time::Duration;

// rust proxy prototype
// async version
#[async_std::main]
async fn main() {
    println!("rs proxy.");

    // config hardcode
    let addr = "0.0.0.0:4040";
    let upstream_addr = "0.0.0.0:4044";

    // init listener
    let ln = TcpListener::bind(addr).await.unwrap();
    ln.incoming()
        .for_each_concurrent(None, |tcp_stream| async move {
            let stream = tcp_stream.unwrap();
            spawn(handle_connection(stream, upstream_addr));
        })
        .await;

    println!("server stop.")
}

async fn handle_connection(stream: TcpStream, upstream_addr: &str) {
    println!("conn handler: handling.");

    // dial upstream connection
    let upstream_conn_fut = TcpStream::connect(upstream_addr);
    let uconn_timeout = Duration::from_millis(15_000);
    let upstream = match future::timeout(uconn_timeout, upstream_conn_fut).await {
        Ok(uconn_res) => match uconn_res {
            Ok(stream) => stream,
            Err(err) => {
                println!("conn handler: upstream conn error: {:?}", err);
                return;
            }
        },
        Err(err) => {
            println!("conn handler: upstream conn timeout error: {:?}", err);
            return;
        }
    };
    // we need read and write for same TcpStream object (concurently)
    // we need pass two mut ref for same object (one as Read and one as Write traits)
    // but it is not allowed by borrow rules, we can have only one mut ref
    // trick solution
    // we make two reference (&TcpStream) allowed by borrow rules
    // for each referenct we get separate mutable reference (&mut &TcpStream)
    // we do not have issue with TcpStream because mutual reference required only for Read/Write traits
    // TcpStream itself do not require mutability
    // TcpStream implements two version of Read/Write traits one for TcpStream another for &TcpStream
    // so when we pass &mut &TcpStream we call &TcpStream Read/Write impl
    // (we also can just clone TcpStream, see prev commit implementation)
    // (clone of TcpStream relativly cheap because it is just wrap to socket file descriptor)
    let (rstream, wstream) = &mut (&stream, &stream);
    let (rupstream, wupstream) = &mut (&upstream, &upstream);

    // forward streams
    if let Err(err) = try_join!(
        forwarder("stream to upstream", rstream, wupstream),
        forwarder("upstream to stream", rupstream, wstream),
    ) {
        println!("conn handler: forwarder err: {}", err)
    };

    println!("conn handler: done.");
}

async fn forwarder(
    name: &str,
    mut rstream: impl Read + Unpin,
    mut wstream: impl Write + Unpin,
) -> Result<(), Error> {
    loop {
        // read
        let mut buffer = [0; 1024];
        let read_len = match rstream.read(&mut buffer).await {
            Ok(0) => {
                println!("forwarder: rstream read 0 bytes, other side close connection");
                return Result::Err(Error::from(ErrorKind::UnexpectedEof));
            }
            Ok(n) => n,
            Err(err) => {
                println!("forwarder: rstream read error: {:?}", err);
                return Result::Err(err);
            }
        };
        _ = name;
        // // print for test
        // let received = std::str::from_utf8(&buffer[0..read_len]).expect("valid utf8");
        // println!("{} read: {:?}", name, received);

        // write
        let write_len = match wstream.write(&buffer[0..read_len]).await {
            Ok(n) => n,
            Err(err) => {
                println!("forwarder: wstream write error: {:?}", err);
                return Result::Err(err);
            }
        };
        _ = write_len;
        // // print for test
        // println!("{} write len {}", name, write_len);
    }
}
