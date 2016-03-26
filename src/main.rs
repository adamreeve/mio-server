#![feature(result_expect)]

extern crate mio;

#[macro_use]
extern crate log;
extern crate env_logger;
extern crate slab;

mod server;
mod connection;

use std::net::SocketAddr;
use std::str::FromStr;

use mio::EventLoop;
use mio::tcp::TcpListener;

use server::Server;


const ADDRESS: &'static str = "127.0.0.1:8081";


fn main() {
    env_logger::init().expect("Failed to initialise logger");

    let addr: SocketAddr = FromStr::from_str(ADDRESS)
        .expect("Failed to parse socket address");

    let server_socket = TcpListener::bind(&addr)
        .expect("Failed to bind to address");

    let mut event_loop: EventLoop<Server> = EventLoop::new()
        .expect("Failed to create event loop");

    let mut server = Server::new(server_socket);
    server.register(&mut event_loop)
        .expect("Failed to register server with event loop");

    info!("Listening on {}", ADDRESS);

    match event_loop.run(&mut server) {
        Ok(_) => (),
        Err(io_error) => {
            error!("Error running event loop: {}", io_error)
        }
    }
}
