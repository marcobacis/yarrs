use std::{
    num::{ParseFloatError, ParseIntError},
    string::FromUtf8Error,
};

use anyhow::anyhow;
use thiserror::Error;

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

impl From<ParseIntError> for FrameParsingError {
    fn from(_value: ParseIntError) -> Self {
        "invalid integer format".into()
    }
}

impl From<ParseFloatError> for FrameParsingError {
    fn from(_value: ParseFloatError) -> Self {
        "invalid double format".into()
    }
}

impl From<std::io::Error> for FrameParsingError {
    fn from(value: std::io::Error) -> Self {
        FrameParsingError::Other(value.into())
    }
}
