use std::io;
use std::net::SocketAddr;

use mio::{EventLoop,Handler,Token,EventSet,PollOpt};
use mio::tcp::{TcpListener,TcpStream};
use slab::Slab;

use connection::Connection;


pub struct Server {
    // Main socket the server listens on
    socket: TcpListener,

    // Token representing the server connection
    token: Token,

    // Set of accepted TCP connections
    connections: Slab<Connection,Token>,
}

impl Server {
    pub fn new(socket: TcpListener) -> Server {
        Server {
            socket: socket,
            token: Token(1),
            connections: Slab::new_starting_at(Token(2), 128),
        }
    }

    pub fn register(&self, event_loop: &mut EventLoop<Server>) -> io::Result<()> {
        event_loop.register(
            &self.socket,
            self.token,
            EventSet::readable(),
            PollOpt::edge()
        ).or_else(|e| {
            error!("Failed to register server: {:?}", e);
            Err(e)
        })
    }

    // Accept a new connection
    fn accept(&mut self, event_loop: &mut EventLoop<Server>) {
        match self.socket.accept() {
            Ok(Some((stream, addr))) => {
                self.add_connection(stream, addr, event_loop);
            }
            Ok(None) => {
                error!("Wouldblock in accept");
            }
            Err(io_error) => {
                error!("Failed to accept new socket: {:?}", io_error);
            }
        }
    }

    fn add_connection(&mut self, stream: TcpStream, addr: SocketAddr, event_loop: &mut EventLoop<Server>) {
        match self.connections.insert_with(|token| {
            Connection::new(stream, addr, token)
        }) {
            Some(token) => {
                // We've inserted a new connection, so now register
                // it with the event loop.
                self.get_connection(token).register(event_loop).unwrap_or_else(|e| {
                    error!("Failed to register new connection for token {:?}: {:?}",
                           token, e);
                    self.connections.remove(token);
                });
            }
            None => {
                error!("Failed to insert new connection");
            }
        }
    }

    fn reset_connection(&mut self, event_loop: &mut EventLoop<Server>, token: Token)
    {
        if self.token == token {
            // We're resetting the server connection, so shutdown the event loop
            event_loop.shutdown();
        } else {
            debug!("Resetting connection for token: {:?}", token);
            self.connections.remove(token);
        }
    }

    fn get_connection<'a>(&'a mut self, token: Token) -> &'a mut Connection {
        &mut self.connections[token]
    }
}

impl Handler for Server {
    type Timeout = ();
    type Message = ();

    fn ready(&mut self, event_loop: &mut EventLoop<Self>, token: Token, events: EventSet) {
        if events.is_error() {
            warn!("Error event for {:?}", token);
            self.reset_connection(event_loop, token);
            return;
        }

        if events.is_hup() {
            debug!("HUP event for {:?}", token);
            self.reset_connection(event_loop, token);
            return;
        }

        if events.is_writable() {
            trace!("Write event for {:?}", token);
            self.get_connection(token).writable().and_then(|_| {
                self.get_connection(token).reregister(event_loop)
            }).unwrap_or_else(|e| {
                error!("Write failed for {:?}: {:?}", token, e);
                self.reset_connection(event_loop, token);
            });
        }

        if events.is_readable() {
            trace!("Read event for {:?}", token);
            if token == self.token {
                self.accept(event_loop);
            } else {
                self.get_connection(token).readable().and_then(|_| {
                    self.get_connection(token).reregister(event_loop)
                }).unwrap_or_else(|e| {
                    error!("Read failed for {:?}: {:?}", token, e);
                    self.reset_connection(event_loop, token);
                });
            }
        }
    }
}
