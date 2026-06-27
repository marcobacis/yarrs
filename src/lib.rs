use thiserror::Error;
use tokio::net::{TcpListener, TcpStream};

use crate::protocol::{connection::Connection, error::FrameParsingError, types::Frame};
pub mod protocol;

#[derive(Error, Debug)]
pub enum ServerError {
    #[error("Invalid command syntax: {0}")]
    CommandInvalidSyntax(String),
    #[error("Command \"{0}\" not available")]
    CommandNotAvailable(String),
    #[error("Invalid data")]
    InvalidSocketData(#[from] FrameParsingError),
    #[error("Generic IO error")]
    ServerIoError(#[from] std::io::Error),
}

pub async fn run(listener: TcpListener) -> Result<(), ServerError> {
    println!("Starting server, listening for connections");
    loop {
        let (mut socket, _) = listener.accept().await?;
        tokio::spawn(async move {
            handle(&mut socket).await;
        });
    }
}

async fn handle(socket: &mut TcpStream) -> Result<(), ServerError> {
    let mut connection = Connection::new(socket);
    while let Ok(Some(message)) = connection.read::<Frame, FrameParsingError>().await {
        let result = handle_message(&message).await?;
        connection.write(&result).await?;
    }
    Ok(())
}

async fn handle_message(message: &Frame) -> Result<Frame, ServerError> {
    let elements = match message {
        Frame::Array(frames) => frames,
        _ => {
            return Err(ServerError::CommandInvalidSyntax(
                "shold be RESP array of bulk strings".to_string(),
            ))
        }
    };

    let mut command = Vec::new();
    for elem in elements {
        match elem {
            Frame::Bulk(s) => command.push(String::from_utf8(s.to_vec()).map_err(|e| {
                ServerError::CommandInvalidSyntax("must be utf8 string?".to_string())
            })?),
            _ => {
                return Err(ServerError::CommandInvalidSyntax(
                    "shold be RESP array of bulk strings".to_string(),
                ))
            }
        };
    }

    if command.len() < 1 {
        return Err(ServerError::CommandInvalidSyntax(
            "missing command name".to_string(),
        ));
    }

    let command_name = command[0].to_lowercase();

    match command_name.as_str() {
        "echo" => {
            if command.len() < 2 {
                Err(ServerError::CommandInvalidSyntax(
                    "missing argument".to_string(),
                ))
            } else {
                Ok(Frame::Simple(command[1].clone()))
            }
        }
        "ping" => Ok(Frame::Simple("PONG".into())),
        cmd => Err(ServerError::CommandNotAvailable(cmd.to_string())),
    }
}
