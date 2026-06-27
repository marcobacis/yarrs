use std::{collections::HashMap, sync::atomic::AtomicU64};

use thiserror::Error;
use tokio::{
    net::{TcpListener, TcpStream},
    select,
    sync::mpsc,
};

use crate::{
    messages::{
        ConnectionMessage::{self},
        Request, ServerMessage,
    },
    resp::{connection::Connection, error::FrameParsingError, types::Frame},
};

pub struct Client {
    pub id: u64,
    pub sender: mpsc::Sender<ServerMessage>,
}

pub struct ServerInfo {
    pub host: String,
    pub port: u16,
}

impl ServerInfo {
    pub fn address(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }
}

pub struct Server {
    pub info: ServerInfo,
    pub receiver: mpsc::Receiver<ConnectionMessage>,
    pub sender: mpsc::Sender<ConnectionMessage>,
    pub clients: HashMap<u64, Client>,
    client_id: AtomicU64,
}

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

impl Server {
    pub fn new(host: String, port: u16) -> Self {
        let (sender, recv) = mpsc::channel::<ConnectionMessage>(10);

        Server {
            info: ServerInfo { host, port },
            receiver: recv,
            sender: sender,
            clients: HashMap::new(),
            client_id: AtomicU64::new(0),
        }
    }

    pub async fn run(&mut self) {
        loop {
            select! {
                Some(command) = self.receiver.recv() => {
                    match command {
                        ConnectionMessage::NewClient(sender) => {
                            let new_id = self.client_id.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            let client = Client {
                                id: new_id,
                                sender: sender.clone()
                            };
                            client.sender.send(ServerMessage::ClientInitialized(new_id)).await;
                            self.clients.insert(new_id, client);
                        },
                        ConnectionMessage::ClientRequest(request) => {
                            let id = request.client_id;
                            if let Some(client)  = self.clients.get(&id) {
                                match handle_message(&request.frame).await {
                                    Ok(response) => client.sender.send(ServerMessage::Data(response)).await,
                                    Err(_) => Ok(()),
                                };
                            }
                        },
                    }
                }
            }
        }
    }
}

pub async fn bind(host: String, port: u16) -> TcpListener {
    TcpListener::bind(format!("{}:{}", host, port))
        .await
        .expect("Couldn't create tcp listener")
}

pub async fn run_listener(listener: &mut TcpListener, sender: mpsc::Sender<ConnectionMessage>) {
    loop {
        let (mut socket, _) = listener.accept().await.unwrap();
        let sender = sender.clone();
        tokio::spawn(async move {
            handle_connection(&mut socket, sender).await;
        });
    }
}

async fn handle_connection(socket: &mut TcpStream, sender: mpsc::Sender<ConnectionMessage>) {
    let (connection_sender, mut connection_receiver) = mpsc::channel::<ServerMessage>(32);

    if let Err(e) = sender
        .send(ConnectionMessage::NewClient(connection_sender.clone()))
        .await
    {
        eprintln!("Error sending new client request: {}", e);
        return;
    }

    let id = match connection_receiver.recv().await {
        Some(ServerMessage::ClientInitialized(id)) => id,
        _ => {
            eprintln!("Error initializing client");
            return;
        }
    };

    let mut connection = Connection::new(socket);
    loop {
        select! {
            Ok(Some(frame)) = connection.read::<Frame, FrameParsingError>() => {
                if let Err(e) = sender.send(ConnectionMessage::ClientRequest(Request {
                    client_id: id,
                    frame,
                })).await {
                    eprintln!("Error sending request: {}", e);
                    return;
                }
            },

            Some(ServerMessage::Data(frame)) = connection_receiver.recv() =>  {
                if let Err(e) = connection.write(&frame).await {
                    eprintln!("Error sending request: {}", e);
                    return;
                }
            }
        };
    }
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
