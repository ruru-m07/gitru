// Qualification-only repair for the frozen HTTP fixture, not provider code.
fn qualification_http_request(
    stream: &mut std::net::TcpStream,
    deadline: std::time::Instant,
) -> std::io::Result<Vec<u8>> {
    use std::io::Read as _;
    use std::io::{Error, ErrorKind};
    const LIMIT: usize = 8192;
    stream.set_nonblocking(false)?;
    let mut request = Vec::new();
    loop {
        let remaining = deadline
            .checked_duration_since(std::time::Instant::now())
            .filter(|value| !value.is_zero())
            .ok_or_else(|| Error::new(ErrorKind::TimedOut, "Fixture request deadline"))?;
        stream.set_read_timeout(Some(remaining))?;
        let mut chunk = [0u8; 1024];
        let count = match stream.read(&mut chunk) {
            Ok(0) => {
                return Err(Error::new(
                    ErrorKind::UnexpectedEof,
                    "Truncated fixture request",
                ));
            }
            Ok(count) => count,
            Err(error) if error.kind() == ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        };
        if request.len() + count > LIMIT {
            return Err(Error::new(
                ErrorKind::InvalidData,
                "Fixture request exceeds bound",
            ));
        }
        request.extend_from_slice(&chunk[..count]);
        if let Some(end) = request.windows(4).position(|bytes| bytes == b"\r\n\r\n") {
            let headers = std::str::from_utf8(&request[..end])
                .map_err(|_| Error::new(ErrorKind::InvalidData, "Invalid fixture headers"))?;
            let mut content_length = None;
            for line in headers.lines().skip(1) {
                let (name, value) = line
                    .split_once(':')
                    .ok_or_else(|| Error::new(ErrorKind::InvalidData, "Invalid fixture header"))?;
                if name.eq_ignore_ascii_case("transfer-encoding") {
                    return Err(Error::new(
                        ErrorKind::InvalidData,
                        "Unexpected chunked fixture request",
                    ));
                }
                if name.eq_ignore_ascii_case("content-length") {
                    if content_length.is_some() {
                        return Err(Error::new(
                            ErrorKind::InvalidData,
                            "Duplicate fixture length",
                        ));
                    }
                    content_length = Some(value.trim().parse::<usize>().map_err(|_| {
                        Error::new(ErrorKind::InvalidData, "Invalid fixture length")
                    })?);
                }
            }
            let expected = (end + 4)
                .checked_add(content_length.unwrap_or(0))
                .filter(|length| *length <= LIMIT)
                .ok_or_else(|| Error::new(ErrorKind::InvalidData, "Fixture body exceeds bound"))?;
            if request.len() == expected {
                return Ok(request);
            }
            if request.len() > expected {
                return Err(Error::new(
                    ErrorKind::InvalidData,
                    "Unexpected pipelined fixture request",
                ));
            }
        }
    }
}

#[cfg(test)]
mod qualification_http_fixture_tests {
    use super::qualification_http_request;
    use std::{
        io::{ErrorKind, Write},
        net::{TcpListener, TcpStream},
        time::{Duration, Instant},
    };

    fn pair() -> (TcpStream, TcpStream) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (server, _) = listener.accept().unwrap();
        // Explicitly reproduce the inherited Windows socket state.
        server.set_nonblocking(true).unwrap();
        (client, server)
    }
    #[test]
    fn fragmented_header_and_body_are_fully_read() {
        let (mut client, mut server) = pair();
        let writer = std::thread::spawn(move || {
            for chunk in [
                b"PATCH /fixture HTTP/1.1\r\nContent-Len".as_slice(),
                b"gth: 7\r\n\r\n{\"x",
                b"\":1}",
            ] {
                client.write_all(chunk).unwrap();
                std::thread::sleep(Duration::from_millis(15));
            }
        });
        let request =
            qualification_http_request(&mut server, Instant::now() + Duration::from_secs(2))
                .unwrap();
        writer.join().unwrap();
        assert_eq!(
            request,
            b"PATCH /fixture HTTP/1.1\r\nContent-Length: 7\r\n\r\n{\"x\":1}"
        );
    }
    #[test]
    fn truncated_request_and_oversized_body_fail() {
        for request in [
            b"GET /fixture HTTP/1.1\r\n".as_slice(),
            b"PATCH / HTTP/1.1\r\nContent-Length: 7\r\n\r\n{}",
        ] {
            let (mut client, mut server) = pair();
            client.write_all(request).unwrap();
            drop(client);
            assert_eq!(
                qualification_http_request(&mut server, Instant::now() + Duration::from_secs(2))
                    .unwrap_err()
                    .kind(),
                ErrorKind::UnexpectedEof
            );
        }
        let (mut client, mut server) = pair();
        client
            .write_all(b"PATCH / HTTP/1.1\r\nContent-Length: 8192\r\n\r\n")
            .unwrap();
        assert_eq!(
            qualification_http_request(&mut server, Instant::now() + Duration::from_secs(2))
                .unwrap_err()
                .kind(),
            ErrorKind::InvalidData
        );
    }
    #[test]
    fn no_bytes_and_expired_deadline_remain_bounded() {
        let (_client, mut server) = pair();
        let start = Instant::now();
        let error =
            qualification_http_request(&mut server, start + Duration::from_millis(30)).unwrap_err();
        assert!(matches!(
            error.kind(),
            ErrorKind::TimedOut | ErrorKind::WouldBlock
        ));
        assert!(start.elapsed() < Duration::from_secs(5));
        assert_eq!(
            qualification_http_request(&mut server, Instant::now())
                .unwrap_err()
                .kind(),
            ErrorKind::TimedOut
        );
    }
}
