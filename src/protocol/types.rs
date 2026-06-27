use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
    io::{Cursor, Read},
    str::{self, FromStr},
};

use bytes::Bytes;

use super::{connection::Message, error::FrameParsingError};

#[derive(Debug, PartialEq, Hash)]
pub enum VerbatimEncoding {
    Text,
    Markdown,
    Other([u8; 3]),
}

impl TryFrom<&[u8]> for VerbatimEncoding {
    type Error = FrameParsingError;

    fn try_from(value: &[u8]) -> Result<Self, Self::Error> {
        match value {
            [b't', b'x', b't'] => Ok(VerbatimEncoding::Text),
            [b'm', b'k', b'd'] => Ok(VerbatimEncoding::Markdown),
            [a, b, c] => Ok(VerbatimEncoding::Other([*a, *b, *c])),
            _ => Err("invalid number of characters for verbatim encoding".into()),
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum Frame {
    Array(Vec<Frame>),
    Attribute(HashMap<Frame, Frame>),
    BigNumber(String),
    Boolean(bool),
    Bulk(Bytes),
    BulkError(String),
    Double(f64),
    Error(String),
    Integer(i64),
    Map(HashMap<Frame, Frame>),
    Null,
    Push(Vec<Frame>),
    Set(HashSet<Frame>),
    Simple(String),
    Verbatim(VerbatimEncoding, String),
}

impl Eq for Frame {}

const ARRAY_PREFIX: u8 = b'*';
const ATTRIBUTE_PREFIX: u8 = b'|';
const BIGNUMBER_PREFIX: u8 = b'(';
const BOOLEAN_PREFIX: u8 = b'#';
const BULK_PREFIX: u8 = b'$';
const BULKERROR_PREFIX: u8 = b'!';
const DOUBLE_PREFIX: u8 = b'.';
const ERROR_PREFIX: u8 = b'-';
const INTEGER_PREFIX: u8 = b':';
const MAP_PREFIX: u8 = b'%';
const NULL_PREFIX: u8 = b'_';
const PUSH_PREFIX: u8 = b'>';
const SET_PREFIX: u8 = b'~';
const SIMPLE_PREFIX: u8 = b'+';
const VERBATIM_PREFIX: u8 = b'=';
const NEWLINE: [u8; 2] = [b'\r', b'\n'];

impl Frame {
    pub fn prefix(&self) -> u8 {
        match self {
            Frame::Array(_) => ARRAY_PREFIX,
            Frame::Attribute(_) => ATTRIBUTE_PREFIX,
            Frame::BigNumber(_) => BIGNUMBER_PREFIX,
            Frame::Boolean(_) => BOOLEAN_PREFIX,
            Frame::Bulk(_) => BULK_PREFIX,
            Frame::BulkError(_) => BULKERROR_PREFIX,
            Frame::Double(_) => DOUBLE_PREFIX,
            Frame::Error(_) => ERROR_PREFIX,
            Frame::Integer(_) => INTEGER_PREFIX,
            Frame::Map(_) => MAP_PREFIX,
            Frame::Null => NULL_PREFIX,
            Frame::Push(_) => PUSH_PREFIX,
            Frame::Set(_) => SET_PREFIX,
            Frame::Simple(_) => SIMPLE_PREFIX,
            Frame::Verbatim(_, _) => VERBATIM_PREFIX,
        }
    }
}

impl Hash for Frame {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.prefix().hash(state);
        match self {
            Frame::Array(data) => data.hash(state),
            Frame::BigNumber(data) => data.hash(state),
            Frame::Boolean(data) => data.hash(state),
            Frame::Bulk(data) => data.hash(state),
            Frame::BulkError(data) => data.hash(state),
            Frame::Double(data) => data.to_string().hash(state),
            Frame::Error(data) => data.hash(state),
            Frame::Integer(data) => data.hash(state),
            Frame::Null => "_\r\n".hash(state),
            Frame::Push(data) => data.hash(state),
            Frame::Simple(data) => data.hash(state),
            Frame::Verbatim(encoding, data) => {
                encoding.hash(state);
                data.hash(state);
            }
            _ => panic!("Invalid frame type to be hashed"),
        }
    }
}

impl Message<Frame, FrameParsingError> for Frame {
    fn parse(buf: &mut Cursor<&[u8]>) -> Result<Frame, FrameParsingError> {
        match read_u8(buf)? {
            SIMPLE_PREFIX => Ok(Frame::Simple(read_line_simple(buf)?)),
            ERROR_PREFIX => Ok(Frame::Error(read_line_simple(buf)?)),
            INTEGER_PREFIX => Ok(Frame::Integer(read_from_line(buf)?)),
            DOUBLE_PREFIX => Ok(Frame::Double(read_from_line(buf)?)),
            BULK_PREFIX => {
                let size = read_from_line::<i32>(buf)?;
                match size {
                    num if num >= 0 => {
                        let data = read_bytes(buf, size as usize)?;
                        Ok(Frame::Bulk(data))
                    }
                    -1 => Ok(Frame::Null),
                    _ => Err("invalid bulk string size".into()),
                }
            }
            NULL_PREFIX => Ok(Frame::Null),
            ARRAY_PREFIX => Ok(Frame::Array(read_array(buf)?)),
            BOOLEAN_PREFIX => match read_u8(buf) {
                Ok(b't') => Ok(Frame::Boolean(true)),
                Ok(b'f') => Ok(Frame::Boolean(false)),
                Ok(_) => Err("invalid character for boolean".into()),
                Err(_) => Err(FrameParsingError::Incomplete),
            },
            BIGNUMBER_PREFIX => Ok(Frame::BigNumber(read_line_simple(buf)?)),
            BULKERROR_PREFIX => {
                let size = read_from_line::<u32>(buf)?;
                let data = read_bytes(buf, size as usize)?;
                Ok(Frame::BulkError(String::from_utf8(data.to_vec())?))
            }
            VERBATIM_PREFIX => {
                let size = read_from_line::<u32>(buf)?;
                let data = read_bytes(buf, (size + 4) as usize)?;
                if data[3] != b':' {
                    return Err("Missing ':' character as 4th byte".into());
                }
                let encoding: VerbatimEncoding = data[..3].try_into()?;
                let content = str::from_utf8(&data[4..])?.to_owned();
                Ok(Frame::Verbatim(encoding, content))
            }
            MAP_PREFIX => Ok(Frame::Map(read_map(buf)?)),
            ATTRIBUTE_PREFIX => Ok(Frame::Attribute(read_map(buf)?)),
            SET_PREFIX => Ok(Frame::Set(HashSet::from_iter(read_array(buf)?))),
            PUSH_PREFIX => Ok(Frame::Push(read_array(buf)?)),
            _ => todo!("Implement error handling"),
        }
    }

    fn check(cursor: &mut Cursor<&[u8]>) -> bool {
        // TODO improve length check method
        match Self::parse(cursor) {
            Ok(_) => true,
            Err(FrameParsingError::Incomplete) => false,
            Err(_) => true,
        }
    }

    fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        match self {
            Frame::Array(frames) => {
                serialize_array(&mut buf, ARRAY_PREFIX, frames);
            }
            Frame::Attribute(hash_map) => serialize_map(&mut buf, ATTRIBUTE_PREFIX, hash_map),
            Frame::BigNumber(n) => serialize_simple_string(&mut buf, BIGNUMBER_PREFIX, n),
            Frame::Boolean(v) => {
                let value = if *v { "t" } else { "f" };
                serialize_simple_string(&mut buf, BOOLEAN_PREFIX, value)
            }
            Frame::Bulk(bytes) => {
                buf.push(BULK_PREFIX);
                buf.extend_from_slice(bytes.len().to_string().as_bytes());
                buf.extend_from_slice(&NEWLINE);
                buf.extend_from_slice(bytes);
                buf.extend_from_slice(&NEWLINE);
            }
            Frame::BulkError(s) => {
                let bytes = s.as_bytes();
                buf.push(BULKERROR_PREFIX);
                buf.extend_from_slice(bytes.len().to_string().as_bytes());
                buf.extend_from_slice(&NEWLINE);
                buf.extend_from_slice(bytes);
                buf.extend_from_slice(&NEWLINE);
            }
            Frame::Double(n) => {
                serialize_simple_string(&mut buf, DOUBLE_PREFIX, &format!("{:+}", n))
            }
            Frame::Error(s) => serialize_simple_string(&mut buf, ERROR_PREFIX, s),
            Frame::Integer(n) => {
                serialize_simple_string(&mut buf, INTEGER_PREFIX, &format!("{:+}", n))
            }
            Frame::Map(hash_map) => serialize_map(&mut buf, MAP_PREFIX, hash_map),
            Frame::Null => {
                buf.push(NULL_PREFIX);
                buf.extend_from_slice(&NEWLINE);
            }
            Frame::Push(frames) => serialize_array(&mut buf, PUSH_PREFIX, frames),
            Frame::Set(hash_set) => serialize_set(&mut buf, SET_PREFIX, hash_set),
            Frame::Simple(s) => serialize_simple_string(&mut buf, SIMPLE_PREFIX, s),
            Frame::Verbatim(enc, s) => {
                let bytes = s.as_bytes();
                let enc = match enc {
                    VerbatimEncoding::Text => &[b't', b'x', b't'],
                    VerbatimEncoding::Markdown => &[b'm', b'k', b'd'],
                    VerbatimEncoding::Other(value) => value,
                };
                buf.push(VERBATIM_PREFIX);
                buf.extend_from_slice(bytes.len().to_string().as_bytes());
                buf.extend_from_slice(&NEWLINE);
                buf.extend_from_slice(enc);
                buf.push(b':');
                buf.extend_from_slice(bytes);
                buf.extend_from_slice(&NEWLINE);
            }
        }
        buf
    }
}

fn read_array(buf: &mut Cursor<&[u8]>) -> Result<Vec<Frame>, FrameParsingError> {
    let size = read_from_line::<u32>(buf)? as usize;
    let mut array = Vec::with_capacity(size);
    for _ in 0..size {
        let frame = Frame::parse(buf)?;
        array.push(frame);
    }
    Ok(array)
}

fn read_map(buf: &mut Cursor<&[u8]>) -> Result<HashMap<Frame, Frame>, FrameParsingError> {
    let size = read_from_line(buf)?;
    let mut array = HashMap::with_capacity(size);
    for _ in 0..size {
        let key = Frame::parse(buf)?;
        let value = Frame::parse(buf)?;
        array.insert(key, value);
    }
    Ok(array)
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

fn read_bytes(buf: &mut Cursor<&[u8]>, size: usize) -> Result<Bytes, FrameParsingError> {
    let start = buf.position() as usize;
    let end = start + size;
    let len = buf.get_ref().len();
    if len < end + 2 {
        return Err(FrameParsingError::Incomplete);
    }
    let data = Bytes::copy_from_slice(&buf.get_ref()[start..(start + size)]);
    buf.set_position((end + 2) as u64);
    Ok(data)
}

fn serialize_simple_string(buf: &mut Vec<u8>, prefix: u8, content: &str) {
    buf.push(prefix);
    buf.extend_from_slice(content.as_bytes());
    buf.extend_from_slice(&NEWLINE);
}

fn serialize_array(buf: &mut Vec<u8>, prefix: u8, frames: &Vec<Frame>) {
    buf.push(prefix);
    buf.extend_from_slice(frames.len().to_string().as_bytes());
    buf.extend_from_slice(&NEWLINE);
    for v in frames {
        buf.extend_from_slice(&v.serialize());
    }
}

fn serialize_set(buf: &mut Vec<u8>, prefix: u8, frames: &HashSet<Frame>) {
    buf.push(prefix);
    buf.extend_from_slice(frames.len().to_string().as_bytes());
    buf.extend_from_slice(&NEWLINE);
    for v in frames {
        buf.extend_from_slice(&v.serialize());
    }
}

fn serialize_map(buf: &mut Vec<u8>, prefix: u8, hash_map: &HashMap<Frame, Frame>) {
    buf.push(prefix);
    buf.extend_from_slice(hash_map.len().to_string().as_bytes());
    buf.extend_from_slice(&NEWLINE);
    for (k, v) in hash_map {
        buf.extend_from_slice(&k.serialize());
        buf.extend_from_slice(&v.serialize());
    }
}

#[cfg(test)]
mod tests {

    use super::Frame;
    use crate::protocol::{connection::Message, types::VerbatimEncoding};
    use rstest::rstest;
    use std::{
        collections::{HashMap, HashSet},
        io::Cursor,
    };

    #[rstest]
    #[case("+OK\r\n", Frame::Simple("OK".to_string()))]
    #[case("+Test String\r\n", Frame::Simple("Test String".to_string()))]
    #[case("+Multi\r\nLine\r\nString\r\n", Frame::Simple("Multi".to_string()))]
    #[case("-Error Example\r\n", Frame::Error("Error Example".to_string()))]
    #[case(":63472834\r\n", Frame::Integer(63472834))]
    #[case(":+1239\r\n", Frame::Integer(1239))]
    #[case(":-20\r\n", Frame::Integer(-20))]
    #[case("$5\r\nhello\r\n", Frame::Bulk("hello".into()))]
    #[case("_\r\n", Frame::Null)]
    #[case("$-1\r\n", Frame::Null)]
    #[case("$0\r\n\r\n", Frame::Bulk("".into()))]
    #[case("*0\r\n", Frame::Array(vec![]))]
    #[case("*2\r\n$5\r\nhello\r\n$5\r\nworld\r\n", Frame::Array(vec![Frame::Bulk("hello".into()), Frame::Bulk("world".into())]))]
    #[case("*3\r\n:1\r\n:2\r\n:3\r\n", Frame::Array(vec![Frame::Integer(1), Frame::Integer(2), Frame::Integer(3)]))]
    #[case("#f\r\n", Frame::Boolean(false))]
    #[case("!30\r\nERROR This is an error message\r\n", Frame::BulkError("ERROR This is an error message".into()))]
    #[case("=19\r\ntxt:Hello from verbatim\r\n", Frame::Verbatim(VerbatimEncoding::Text, "Hello from verbatim".into()))]
    #[case("%2\r\n+first\r\n:1\r\n+second\r\n:2\r\n", Frame::Map(HashMap::from([(Frame::Simple("first".into()), Frame::Integer(1)),(Frame::Simple("second".into()), Frame::Integer(2))])))]
    #[case("|1\r\n+third\r\n:3\r\n", Frame::Attribute(HashMap::from([(Frame::Simple("third".into()), Frame::Integer(3))])))]
    #[case("~3\r\n:1\r\n:2\r\n:3\r\n", Frame::Set(HashSet::from([Frame::Integer(1), Frame::Integer(2), Frame::Integer(3)])))]
    fn test_parse_success(#[case] input: &str, #[case] expected: Frame) {
        let mut cursor = Cursor::new(input.as_bytes());
        let result = Frame::parse(&mut cursor);
        assert_eq!(expected, result.unwrap());
    }

    #[rstest]
    #[case(".1032.34\r\n", 1032.34)]
    #[case(".+834.234\r\n", 834.234)]
    #[case(".-20.12\r\n",  -20.12)]
    #[case(".1e-1\r\n", 0.1)]
    fn test_parse_double_success(#[case] input: &str, #[case] expected: f64) {
        let mut cursor = Cursor::new(input.as_bytes());
        let result = Frame::parse(&mut cursor);
        assert!(matches!(result, Ok(Frame::Double(x)) if (x - expected).abs() < f64::EPSILON));
    }

    #[rstest]
    #[case("")]
    #[case("+OK\r")]
    #[case("+Test")]
    #[case("-Err")]
    #[case(":129")]
    #[case(".123.34\r")]
    #[case("$10\r\nnotenough\r\n")]
    #[case("!50\r\nERROR not enough text\r\n")]
    #[case("=40\r\ntxt:Hello from verbatim\r\n")]
    #[case("%2\r\n+first\r\n:1\r\n")]
    #[case("%2\r\n+first\r\n")]
    #[case("~3\r\n:1\r\n:2\r\n")]
    fn test_parse_incomplete(#[case] input: &str) {
        use crate::protocol::error::FrameParsingError;

        let mut cursor = Cursor::new(input.as_bytes());
        let enough = Frame::check(&mut cursor);
        assert!(!enough);

        cursor.set_position(0);

        let result = Frame::parse(&mut cursor);
        assert!(matches!(result, Err(FrameParsingError::Incomplete)));
    }

    #[rstest]
    #[case(":13472.2348\r\n")]
    #[case(":pasdufgskldfg\r\n")]
    #[case(".str\r\n")]
    #[case(".*234950.45&\r\n")]
    #[case("#c\r\n")]
    #[case("=19\r\ntxtH:ello from verbatim\r\n")]
    #[case("~-34\r\n")]
    #[case("~a\r\n")]
    fn test_parse_invalid(#[case] input: &str) {
        use crate::protocol::error::FrameParsingError;

        let mut cursor = Cursor::new(input.as_bytes());
        let result = Frame::parse(&mut cursor);
        assert!(matches!(result, Err(FrameParsingError::Other(_))));
    }

    #[test]
    fn test_serialize_parse_roundtrip() {
        let frames = vec![
            Frame::Simple("OK".into()),
            Frame::Error("Error".into()),
            Frame::Integer(123),
            Frame::Double(123.456),
            Frame::Bulk("hello".into()),
            Frame::BulkError("error".into()),
            Frame::Null,
            Frame::Array(vec![Frame::Integer(1), Frame::Integer(2)]),
            Frame::Boolean(true),
            Frame::Boolean(false),
            Frame::BigNumber("12345678901234567890".into()),
            Frame::Verbatim(VerbatimEncoding::Text, "hello".into()),
            Frame::Map(HashMap::from([(
                Frame::Simple("k".into()),
                Frame::Integer(1),
            )])),
            Frame::Attribute(HashMap::from([(
                Frame::Simple("k".into()),
                Frame::Integer(1),
            )])),
            Frame::Set(HashSet::from([Frame::Integer(1), Frame::Integer(2)])),
            Frame::Push(vec![Frame::Integer(1), Frame::Integer(2)]),
        ];

        for frame in frames {
            let serialized = frame.serialize();
            let mut cursor = Cursor::new(serialized.as_slice());
            let parsed = Frame::parse(&mut cursor).unwrap();
            assert_eq!(frame, parsed);
        }
    }
}
