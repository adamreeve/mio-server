use std::fmt;
use std::io;
use std::io::{Read,Write,Error,ErrorKind};
use std::net::SocketAddr;
use std::string::FromUtf8Error;

use mio::{EventLoop,Token,EventSet,PollOpt};
use mio::tcp::TcpStream;

use server::Server;

pub struct Connection {
    socket: TcpStream,
    address: SocketAddr,
    token: Token,
    interest: EventSet,
    send_queue: Vec<Vec<u8>>,
}

impl Connection {
    pub fn new(socket: TcpStream, address: SocketAddr, token: Token) -> Connection {
        let connection = Connection {
            socket: socket,
            address: address,
            token: token,
            interest: EventSet::hup() | EventSet::readable(),
            send_queue: Vec::new(),
        };
        info!("New connection: {:?}", connection);
        connection
    }

    pub fn register(&mut self, event_loop: &mut EventLoop<Server>) -> io::Result<()> {
        event_loop.register(
            &self.socket,
            self.token,
            self.interest,
            PollOpt::edge() | PollOpt::oneshot()
        ).or_else(|e| {
            error!("Failed to register server: {:?}", e);
            Err(e)
        })
    }

    pub fn reregister(&mut self, event_loop: &mut EventLoop<Server>) -> io::Result<()> {
        event_loop.reregister(
            &self.socket,
            self.token,
            self.interest,
            PollOpt::edge() | PollOpt::oneshot()
        ).or_else(|e| {
            error!("Failed to register server: {:?}", e);
            Err(e)
        })
    }

    pub fn readable(&mut self) -> io::Result<()> {
        trace!("Readable event for {:?}", self);
        let mut buf = [0u8; 1024];

        let bytes = match self.socket.read(&mut buf) {
            Ok(n) => n,
            Err(e) => return Err(e),
        };

        let message: Vec<u8> = buf.iter()
            .take(bytes)
            .take_while(|&b| {*b != 0u8})
            .cloned().collect();

        // Try and convert the message to a string so we can log it.
        // This moves ownership of the message bytes.
        let message_length = message.len();
        let _: Result<(),FromUtf8Error> = String::from_utf8(message).and_then(|m| {
            info!("{:?} received utf8 message with length {}: {:?}",
                  self, message_length, m);
            Ok(m.into_bytes())
        }).or_else(|e| {
            // If it's not valid UTF8, get the bytes back out of
            // the error and send the message anyway. This means
            // we can avoid copying the original message.
            info!("{:?} received non-utf8 message with length {}",
                  self, message_length);
            Ok(e.into_bytes())
        }).and_then(|m| {
            self.send(m);
            Ok(())
        });

        Ok(())
    }

    pub fn send(&mut self, message: Vec<u8>) {
        self.send_queue.push(message);
        self.interest = self.interest | EventSet::writable()
    }

    pub fn writable(&mut self) -> io::Result<()> {
        trace!("Writable event for {:?}", self);

        let _ = self.send_queue.pop()
            .ok_or(Error::new(ErrorKind::Other, "No messages in send queue"))
            .and_then(|message| {
                match self.socket.write(&*message) {
                    Ok(n) => {
                        info!("Wrote {} bytes", n);
                        Ok(())
                    }
                    Err(e) => {
                        error!("Error writing to socket: {:?}", e);
                        Err(e)
                    }
                }
            });

        if self.send_queue.is_empty() {
            self.interest.remove(EventSet::writable());
        }

        Ok(())
    }
}

impl fmt::Debug for Connection {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Connection {{ token: {:?}, address: {:?} }}", self.token, self.address)
    }
}
