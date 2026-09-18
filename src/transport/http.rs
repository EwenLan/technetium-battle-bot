use std::io::{self, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::time::Duration;

use crate::protocol::empty_response;
use crate::rules::build::BuildMask;
use crate::rules::constants::{
    EMPTY_COUNT, HTTP_BAD_REQUEST, HTTP_METHOD_NOT_ALLOWED, HTTP_OK, HTTP_PAYLOAD_TOO_LARGE,
    HTTP_READ_TIMEOUT_MS, MAX_HTTP_BODY_BYTES, MAX_HTTP_HEADER_BYTES, ZERO_BYTE,
};
use crate::runtime::Session;

const READ_BUFFER_BYTES: usize = 4096;
const HEADER_TERMINATOR: &[u8] = b"\r\n\r\n";
const HEADER_TERMINATOR_BYTES: usize = 4;

pub fn serve(port: u16) -> io::Result<()> {
    let listener = TcpListener::bind(("0.0.0.0", port))?;
    let mut session = Session::new(BuildMask::load_from_env()?);
    for connection in listener.incoming() {
        match connection {
            Ok(mut stream) => {
                if let Err(error) = handle_connection(&mut stream, &mut session) {
                    eprintln!("http connection: {error}");
                }
            }
            Err(error) => eprintln!("http accept: {error}"),
        }
    }
    Ok(())
}

fn handle_connection(stream: &mut TcpStream, session: &mut Session) -> io::Result<()> {
    stream.set_read_timeout(Some(Duration::from_millis(HTTP_READ_TIMEOUT_MS)))?;
    let result = read_request(stream);
    let (status, body) = match result {
        Ok(request) if request.method == "POST" => (HTTP_OK, session.handle(&request.body)),
        Ok(_) => (HTTP_METHOD_NOT_ALLOWED, empty_response()),
        Err(error) if error.kind() == io::ErrorKind::InvalidData => {
            (HTTP_PAYLOAD_TOO_LARGE, empty_response())
        }
        Err(_) => (HTTP_BAD_REQUEST, empty_response()),
    };
    write_response(stream, status, &body)
}

struct HttpRequest {
    method: String,
    body: Vec<u8>,
}

fn read_request(stream: &mut TcpStream) -> io::Result<HttpRequest> {
    let mut bytes = read_head(stream)?;
    let end = header_end(&bytes).ok_or_else(invalid_request)?;
    let (method, content_length) = parse_head(&bytes[..end])?;
    let body_start = end + HEADER_TERMINATOR_BYTES;
    if content_length > MAX_HTTP_BODY_BYTES {
        return Err(oversized_request());
    }
    read_body(stream, &mut bytes, body_start + content_length)?;
    Ok(HttpRequest {
        method,
        body: bytes[body_start..body_start + content_length].to_vec(),
    })
}

fn read_head(stream: &mut TcpStream) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    let mut buffer = [ZERO_BYTE; READ_BUFFER_BYTES];
    while header_end(&bytes).is_none() {
        if bytes.len() > MAX_HTTP_HEADER_BYTES {
            return Err(oversized_request());
        }
        let count = stream.read(&mut buffer)?;
        if count == EMPTY_COUNT {
            return Err(invalid_request());
        }
        bytes.extend_from_slice(&buffer[..count]);
    }
    Ok(bytes)
}

fn read_body(stream: &mut TcpStream, bytes: &mut Vec<u8>, needed: usize) -> io::Result<()> {
    let mut buffer = [ZERO_BYTE; READ_BUFFER_BYTES];
    while bytes.len() < needed {
        let count = stream.read(&mut buffer)?;
        if count == EMPTY_COUNT {
            return Err(invalid_request());
        }
        bytes.extend_from_slice(&buffer[..count]);
    }
    Ok(())
}

fn parse_head(bytes: &[u8]) -> io::Result<(String, usize)> {
    let head = std::str::from_utf8(bytes).map_err(|_| invalid_request())?;
    let mut lines = head.split("\r\n");
    let first = lines.next().ok_or_else(invalid_request)?;
    let method = first
        .split_whitespace()
        .next()
        .ok_or_else(invalid_request)?;
    let length = lines.find_map(content_length).ok_or_else(invalid_request)?;
    Ok((method.to_owned(), length))
}

fn content_length(line: &str) -> Option<usize> {
    let (name, value) = line.split_once(':')?;
    if !name.eq_ignore_ascii_case("content-length") {
        return None;
    }
    value.trim().parse().ok()
}

fn header_end(bytes: &[u8]) -> Option<usize> {
    bytes
        .windows(HEADER_TERMINATOR_BYTES)
        .position(|window| window == HEADER_TERMINATOR)
}

fn write_response(stream: &mut TcpStream, status: u16, body: &[u8]) -> io::Result<()> {
    let reason = match status {
        HTTP_OK => "OK",
        HTTP_METHOD_NOT_ALLOWED => "Method Not Allowed",
        HTTP_PAYLOAD_TOO_LARGE => "Payload Too Large",
        _ => "Bad Request",
    };
    let header = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    stream.write_all(header.as_bytes())?;
    stream.write_all(body)
}

fn invalid_request() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, "invalid HTTP request")
}

fn oversized_request() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "HTTP request too large")
}
