use std::{
    io::{Cursor, Read},
    str::FromStr,
};

use bytes::Bytes;
use error::FrameParsingError;

pub mod error;

#[derive(Debug, PartialEq)]
pub enum Frame {
    Simple(String),
    Error(String),
    Integer(i64),
    Double(f64),
    Bulk(Bytes),
    Null,
}

impl Frame {
    pub fn parse(buf: &mut Cursor<&[u8]>) -> Result<Frame, FrameParsingError> {
        match read_u8(buf)? {
            b'+' => Ok(Frame::Simple(read_line_simple(buf)?)),
            b'-' => Ok(Frame::Error(read_line_simple(buf)?)),
            b':' => Ok(Frame::Integer(read_from_line(buf)?)),
            b'.' => Ok(Frame::Double(read_from_line(buf)?)),
            b'$' => {
                let size = read_from_line::<i32>(buf)?;
                match size {
                    num if num >= 0 => {
                        let size = size as usize;
                        let start = buf.position() as usize;
                        let end = start + size;
                        let len = buf.get_ref().len();
                        if len < end + 2 {
                            return Err(FrameParsingError::Incomplete);
                        }
                        let data = Bytes::copy_from_slice(&buf.get_ref()[start..(start + size)]);
                        buf.set_position((end + 2) as u64);
                        Ok(Frame::Bulk(data))
                    }
                    -1 => Ok(Frame::Null),
                    _ => Err("invalid bulk string size".into()),
                }
            }
            b'_' => Ok(Frame::Null),

            b'*' => todo!(), // Arrays
            b'#' => todo!(), // Booleans
            b'(' => todo!(), // Big number
            b'!' => todo!(), // Bulk errors
            b'=' => todo!(), // Verbatim string
            b'%' => todo!(), // Map
            b'|' => todo!(), // Attribute
            b'>' => todo!(), // Push
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

fn read_line_simple(buf: &mut Cursor<&[u8]>) -> Result<String, FrameParsingError> {
    let line = read_line(buf)?.to_vec();
    Ok(String::from_utf8(line)?)
}

fn read_from_line<T>(buf: &mut Cursor<&[u8]>) -> Result<T, FrameParsingError>
where
    T: FromStr,
    T::Err: Into<FrameParsingError>,
{
    let line = read_line(buf)?.to_vec();
    let value = String::from_utf8(line)?.parse().map_err(Into::into)?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use crate::protocol::FrameParsingError;

    use super::Frame;
    use rstest::rstest;
    use std::{f64::EPSILON, io::Cursor};

    #[rstest]
    #[case("+OK\r\n", Frame::Simple("OK".to_string()))]
    #[case("+Test String\r\n", Frame::Simple("Test String".to_string()))]
    // Read only first line for simple string
    #[case("+Multi\r\nLine\r\nString\r\n", Frame::Simple("Multi".to_string()))]
    #[case("-Error Example\r\n", Frame::Error("Error Example".to_string()))]
    #[case(":63472834\r\n", Frame::Integer(63472834))]
    #[case(":+1239\r\n", Frame::Integer(1239))]
    #[case(":-20\r\n", Frame::Integer(-20))]
    #[case("$5\r\nhello\r\n", Frame::Bulk("hello".into()))]
    #[case("_\r\n", Frame::Null)]
    #[case("$-1\r\n", Frame::Null)]
    #[case("$0\r\n\r\n", Frame::Bulk("".into()))]
    fn test_parse_success(#[case] input: &str, #[case] expected: Frame) {
        let mut cursor = Cursor::new(input.as_bytes());
        let result = Frame::parse(&mut cursor);
        assert_eq!(expected, result.unwrap());
    }

    #[rstest]
    #[case(".1032.34\r\n", 1032.34)]
    #[case(".+834.234\r\n", 834.324)]
    #[case(".-20.12\r\n",  -20.12)]
    #[case(".1e-1\r\n", 0.1)]
    fn test_parse_double_success(#[case] input: &str, #[case] expected: f64) {
        let mut cursor = Cursor::new(input.as_bytes());
        let result = Frame::parse(&mut cursor);
        dbg!(&result);
        assert!(matches!(result, Ok(Frame::Double(x)) if (x - expected) < EPSILON));
    }

    #[rstest]
    #[case("")]
    #[case("+OK\r")]
    #[case("+Test")]
    #[case("-Err")]
    #[case(":129")]
    #[case(".123.34\r")]
    #[case("$10\r\nnotenough\r\n")]
    fn test_parse_incomplete(#[case] input: &str) {
        let mut cursor = Cursor::new(input.as_bytes());
        let result = Frame::parse(&mut cursor);
        assert!(matches!(result, Err(FrameParsingError::Incomplete)));
    }

    #[rstest]
    #[case(":13472.2348\r\n")]
    #[case(":pasdufgskldfg\r\n")]
    #[case(".str\r\n")]
    #[case(".*234950.45&\r\n")]
    fn test_parse_invalid(#[case] input: &str) {
        let mut cursor = Cursor::new(input.as_bytes());
        let result = Frame::parse(&mut cursor);
        assert!(matches!(result, Err(FrameParsingError::Other(_))));
    }
}
