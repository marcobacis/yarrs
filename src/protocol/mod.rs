use std::{
    io::{Cursor, Read},
    string::FromUtf8Error,
};

use anyhow::anyhow;
use thiserror::Error;

#[derive(Eq, PartialEq, Debug)]
pub enum Frame {
    Simple(String),
}

impl Frame {
    pub fn parse(buf: &mut Cursor<&[u8]>) -> Result<Frame, FrameParsingError> {
        match read_u8(buf)? {
            b'+' => {
                let line = read_line(buf)?.to_vec();
                let string = String::from_utf8(line)?;
                Ok(Frame::Simple(string))
            }
            _ => todo!("Implement error handling"),
        }
    }
}

fn read_u8(buf: &mut Cursor<&[u8]>) -> Result<u8, FrameParsingError> {
    let mut byte = [0];
    match buf.read_exact(&mut byte) {
        Ok(_) => Ok(byte[0]),
        Err(_) => Err(FrameParsingError::Incomplete),
    }
}

// Read the first line (ending with \r\n) from the buffer
fn read_line<'a>(buf: &mut Cursor<&'a [u8]>) -> Result<&'a [u8], FrameParsingError> {
    let start = buf.position() as usize;
    let end = buf.get_ref().len() - 1;

    for i in start..end {
        if buf.get_ref()[i] == b'\r' && buf.get_ref()[i + 1] == b'\n' {
            // "Consumes the line"
            buf.set_position((i + 2) as u64);
            return Ok(&buf.get_ref()[start..i]);
        }
    }
    Err(FrameParsingError::Incomplete)
}

#[derive(Error, Debug)]
pub enum FrameParsingError {
    #[error("Incomplete buffer to parse message")]
    Incomplete,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

impl From<String> for FrameParsingError {
    fn from(value: String) -> Self {
        FrameParsingError::Other(anyhow!(value))
    }
}

impl From<&str> for FrameParsingError {
    fn from(value: &str) -> Self {
        value.to_string().into()
    }
}

impl From<FromUtf8Error> for FrameParsingError {
    fn from(_value: FromUtf8Error) -> Self {
        "invalid frame format".into()
    }
}

impl From<std::io::Error> for FrameParsingError {
    fn from(value: std::io::Error) -> Self {
        FrameParsingError::Other(value.into())
    }
}

#[cfg(test)]
mod tests {
    use crate::protocol::FrameParsingError;

    use super::Frame;
    use rstest::rstest;
    use std::io::Cursor;

    #[rstest]
    #[case("+OK\r\n", Frame::Simple("OK".to_string()))]
    #[case("+Test String\r\n", Frame::Simple("Test String".to_string()))]
    // Read only first line for simple string
    #[case("+Multi\r\nLine\r\nString\r\n", Frame::Simple("Multi".to_string()))]
    fn test_parse_success(#[case] input: &str, #[case] expected: Frame) {
        let mut cursor = Cursor::new(input.as_bytes());
        let result = Frame::parse(&mut cursor);
        assert_eq!(expected, result.unwrap());
    }

    #[rstest]
    #[case("")]
    #[case("+OK\r")]
    #[case("+Test")]
    fn test_parse_incomplete(#[case] input: &str) {
        let mut cursor = Cursor::new(input.as_bytes());
        let result = Frame::parse(&mut cursor);
        assert!(matches!(result, Err(FrameParsingError::Incomplete)));
    }
}
