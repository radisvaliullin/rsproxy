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
    // address of server where proxy forward stream
    let target_server_addr = "0.0.0.0:4044";

    // init listener
    let ln = TcpListener::bind(addr).await.unwrap();
    ln.incoming()
        .for_each_concurrent(None, |tcp_stream| async move {
            let stream = tcp_stream.unwrap();
            spawn(handle_session(stream, target_server_addr));
        })
        .await;

    println!("server stop.")
}

async fn handle_session(stream: TcpStream, server_addr: &str) {
    println!("conn handler: handling.");

    // dial connection to server
    let server_conn_fut = TcpStream::connect(server_addr);
    let sconn_timeout = Duration::from_millis(15_000);
    let server_stream = match future::timeout(sconn_timeout, server_conn_fut).await {
        Ok(conn_res) => match conn_res {
            Ok(stream) => stream,
            Err(err) => {
                println!("conn handler: server conn error: {:?}", err);
                return;
            }
        },
        Err(err) => {
            println!("conn handler: server conn timeout error: {:?}", err);
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
    let (rsrvstream, wsrvstream) = &mut (&server_stream, &server_stream);

    // forward streams
    if let Err(err) = try_join!(
        forwarder("client to server", rstream, wsrvstream),
        forwarder("server to client", rsrvstream, wstream),
    ) {
        println!("conn handler: forwarder err: {}", err)
    };

    println!("conn handler: done.");
}

// forwarder
// forward from upstream to downstream
async fn forwarder(
    name: &str,
    mut upstream: impl Read + Unpin,
    mut downstream: impl Write + Unpin,
) -> Result<(), Error> {
    loop {
        // read
        let mut buffer = [0; 1024];
        let read_len = match upstream.read(&mut buffer).await {
            Ok(0) => {
                println!("forwarder: upstream read 0 bytes, other side close connection");
                return Result::Err(Error::from(ErrorKind::UnexpectedEof));
            }
            Ok(n) => n,
            Err(err) => {
                println!("forwarder: upstream read error: {:?}", err);
                return Result::Err(err);
            }
        };
        _ = name;
        // // print for test
        // let received = std::str::from_utf8(&buffer[0..read_len]).expect("valid utf8");
        // println!("{} read: {:?}", name, received);

        // write
        let write_len = match downstream.write(&buffer[0..read_len]).await {
            Ok(n) => n,
            Err(err) => {
                println!("forwarder: downstream write error: {:?}", err);
                return Result::Err(err);
            }
        };
        _ = write_len;
        // // print for test
        // println!("{} write len {}", name, write_len);
    }
}
