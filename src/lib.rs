#[macro_use]
mod iter;

use iter::Bytes;
use std::{result, str};
use url::{self, Url};

const META_MAX_LENGTH: usize = 1024;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum Error {
    NewLine,
    InvalidUtf8(str::Utf8Error),
    ParseUrl(url::ParseError),
    ResponseHeader,
    Status,
}

impl From<url::ParseError> for Error {
    fn from(err: url::ParseError) -> Self {
        Error::ParseUrl(err)
    }
}

impl From<str::Utf8Error> for Error {
    fn from(err: str::Utf8Error) -> Self {
        Error::InvalidUtf8(err)
    }
}

pub type Result<T> = result::Result<Status<T>, Error>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status<T> {
    Complete(T),
    Partial,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Request {
    pub url: Option<Url>,
}

impl Request {
    #[inline]
    pub fn new() -> Self {
        Self { url: None }
    }

    pub fn parse(&mut self, buf: &[u8]) -> Result<usize> {
        let mut bytes = Bytes::new(buf);
        complete!(skip_empty_lines(&mut bytes));

        let start = bytes.pos;
        let end = complete!(next_line(&mut bytes));

        let s = unsafe { str::from_utf8_unchecked(&bytes[start..end]) };
        self.url = Some(Url::parse(s)?);

        Ok(Status::Complete(bytes.pos))
    }
}

#[derive(Debug, PartialEq, Eq)]
pub struct Response {
    pub status: Option<u16>,
    pub meta: Option<String>,
}

impl Response {
    #[inline]
    pub fn new() -> Self {
        Self {
            status: None,
            meta: None,
        }
    }

    pub fn parse(&mut self, buf: &[u8]) -> Result<()> {
        let mut bytes = Bytes::new(buf);
        self.status = Some(complete!(parse_status(&mut bytes)));

        expect!(bytes.next() == b' ' => Err(Error::ResponseHeader));

        let start = bytes.pos;
        let end = complete!(next_line_limit(&mut bytes, META_MAX_LENGTH));
        self.meta = Some(String::from(str::from_utf8(&bytes[start..end])?));

        Ok(Status::Complete(()))
    }
}

#[inline]
fn skip_empty_lines(bytes: &mut Bytes) -> Result<()> {
    loop {
        match bytes.peek() {
            Some(b'\r') => {
                unsafe {
                    bytes.bump();
                }

                expect!(bytes.next() == b'\n' => Err(Error::NewLine));
            }
            Some(b'\n') => unsafe {
                bytes.bump();
            },
            Some(..) => return Ok(Status::Complete(())),
            None => return Ok(Status::Partial),
        }
    }
}

#[inline]
fn next_line(bytes: &mut Bytes) -> Result<usize> {
    next_line_inner(bytes, None)
}

#[inline]
fn next_line_limit(bytes: &mut Bytes, limit: usize) -> Result<usize> {
    next_line_inner(bytes, Some(limit))
}

#[inline]
fn next_line_inner(bytes: &mut Bytes, limit: Option<usize>) -> Result<usize> {
    let start = bytes.pos;
    loop {
        match bytes.peek() {
            Some(b'\r') => {
                unsafe {
                    bytes.bump();
                }

                match next!(bytes) {
                    b'\n' => return Ok(Status::Complete(bytes.pos - 2)),
                    _ => return Err(Error::NewLine),
                }
            }
            Some(b'\n') => {
                unsafe {
                    bytes.bump();
                }

                return Ok(Status::Complete(bytes.pos - 1));
            }
            Some(..) => unsafe {
                if let Some(limit) = limit {
                    if bytes.pos - start + 1 > limit {
                        return Err(Error::NewLine);
                    }
                }

                bytes.bump();
            },
            None => return Ok(Status::Partial),
        }
    }
}

#[inline]
fn parse_status(bytes: &mut Bytes) -> Result<u16> {
    let tens = expect!(bytes.next() == b'0'..=b'9' => Err(Error::Status));
    let ones = expect!(bytes.next() == b'0'..=b'9' => Err(Error::Status));
    let result = ((tens - b'0') as u16 * 10) + (ones - b'0') as u16;
    Ok(Status::Complete(result as u16))
}

#[cfg(test)]
mod test {
    use super::*;
    use url::Host;

    #[test]
    fn test_skip_empty_lines() {
        let mut bytes = Bytes::new(b"\r\n\r\ngemini://example.com");

        assert_eq!(skip_empty_lines(&mut bytes), Ok(Status::Complete(())));
        assert_eq!(bytes.pos, 4);

        let mut bytes = Bytes::new(b"\r\n\r\n");

        assert_eq!(skip_empty_lines(&mut bytes), Ok(Status::Partial));
        assert_eq!(bytes.pos, 4);

        let mut bytes = Bytes::new(b"\r\n\r");

        assert_eq!(skip_empty_lines(&mut bytes), Ok(Status::Partial));
        assert_eq!(bytes.pos, 3);

        let mut bytes = Bytes::new(b"\r\n\ra");

        assert_eq!(skip_empty_lines(&mut bytes), Err(Error::NewLine));
    }

    #[test]
    fn test_next_line() {
        let mut bytes = Bytes::new(b"gemini://a.com\r\n");

        assert_eq!(next_line(&mut bytes), Ok(Status::Complete(14)));
        assert_eq!(bytes.pos, 16);

        let mut bytes = Bytes::new(b"gemini://a.com\n");

        assert_eq!(next_line(&mut bytes), Ok(Status::Complete(14)));
        assert_eq!(bytes.pos, 15);

        let mut bytes = Bytes::new(b"gemini://a.com");

        assert_eq!(next_line(&mut bytes), Ok(Status::Partial));

        let mut bytes = Bytes::new(b"gemini://a.com\r\x00");

        assert_eq!(next_line(&mut bytes), Err(Error::NewLine));
    }

    #[test]
    fn test_next_line_limit() {
        let mut bytes = Bytes::new(b"text\r");
        assert_eq!(next_line_limit(&mut bytes, 3), Err(Error::NewLine));
    }

    #[test]
    fn test_request_parse() {
        let buf = b"gemini://example.com\r\n";
        let mut req = Request::new();
        req.parse(buf).unwrap();
        let url = req.url.unwrap();
        assert_eq!(url.scheme(), "gemini");
        assert_eq!(url.host(), Some(Host::Domain("example.com")));

        let buf = b"gemini://example.com";
        let mut req = Request::new();
        assert_eq!(req.parse(buf), Ok(Status::Partial));

        let buf = b"gemini://example.com\r\x00";
        let mut req = Request::new();
        assert_eq!(req.parse(buf), Err(Error::NewLine));
    }

    #[test]
    fn test_response_parse() {
        let buf = b"20 metadata\r\n";
        let mut res = Response::new();
        res.parse(buf).unwrap();
        assert_eq!(res.status, Some(20));
        assert_eq!(res.meta, Some("metadata".to_string()));

        let buf = b"20 metadata";
        let mut res = Response::new();
        assert_eq!(res.parse(buf), Ok(Status::Partial));

        let buf = b"20 metadata\ra";
        let mut res = Response::new();
        assert_eq!(res.parse(buf), Err(Error::NewLine));
    }

    #[test]
    fn test_parse_status() {
        let mut bytes = Bytes::new(b"10");
        assert_eq!(parse_status(&mut bytes), Ok(Status::Complete(10)));
        assert_eq!(bytes.pos, 2);

        let mut bytes = Bytes::new(b"1");
        assert_eq!(parse_status(&mut bytes), Ok(Status::Partial));

        let mut bytes = Bytes::new(b"a0");
        assert_eq!(parse_status(&mut bytes), Err(Error::Status));
    }
}
