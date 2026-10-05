//! One request, one reply. `hookctl` and the tests use it.

use crate::endpoint::Endpoint;
use crate::protocol::{Request, Response};
use std::io::{self, BufRead, BufReader, Write};
use std::net::{Ipv4Addr, TcpStream};
use std::os::unix::net::UnixStream;
use std::time::Duration;

pub fn send(endpoint: &Endpoint, request: &Request, timeout: Duration) -> io::Result<Response> {
    match endpoint {
        Endpoint::Unix(path) => {
            let stream = UnixStream::connect(path)?;
            stream.set_read_timeout(Some(timeout))?;
            stream.set_write_timeout(Some(timeout))?;
            let reader = stream.try_clone()?;
            exchange(reader, stream, request)
        }
        Endpoint::Tcp(port) => {
            let stream = TcpStream::connect((Ipv4Addr::LOCALHOST, *port))?;
            stream.set_read_timeout(Some(timeout))?;
            stream.set_write_timeout(Some(timeout))?;
            let reader = stream.try_clone()?;
            exchange(reader, stream, request)
        }
    }
}

fn exchange(
    reader: impl io::Read,
    mut writer: impl Write,
    request: &Request,
) -> io::Result<Response> {
    writeln!(writer, "{}", request.to_line())?;
    writer.flush()?;
    let mut line = String::new();
    if BufReader::new(reader).read_line(&mut line)? == 0 {
        return Err(io::Error::new(
            io::ErrorKind::UnexpectedEof,
            "the hook closed the connection without a reply",
        ));
    }
    Response::parse(line.trim()).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}
