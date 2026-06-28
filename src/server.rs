use std::{collections::HashMap, sync::atomic::AtomicU64};

use thiserror::Error;
use tokio::{select, sync::mpsc};

use crate::{
    command::{echo, ping},
    messages::{
        ConnectionMessage::{self},
        Request, ServerMessage,
    },
    resp::types::Frame,
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

#[derive(Error, Debug, PartialEq)]
pub enum ServerError {
    #[error("Invalid command syntax: {0}")]
    CommandInvalidSyntax(String),
    #[error("Command \"{0}\" not available")]
    CommandNotAvailable(String),
    #[error("Generic IO error")]
    ServerIoError,
}

impl Server {
    pub fn new(host: String, port: u16) -> Self {
        let (sender, recv) = mpsc::channel::<ConnectionMessage>(10);

        Server {
            info: ServerInfo { host, port },
            receiver: recv,
            sender,
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
                            if let Err(e) = client.sender.send(ServerMessage::ClientInitialized(new_id)).await {
                                eprintln!("Error sending new client id back to client: {}", e);
                            }
                            self.clients.insert(new_id, client);
                        },
                        ConnectionMessage::ClientRequest(request) => {
                            if let Err(e) = self.handle_message(&request).await {
                                eprintln!("Error handling message : {}", e);
                            };
                        },
                    }
                }
            }
        }
    }

    async fn handle_message(self: &Self, request: &Request) -> Result<(), ServerError> {
        let elements = match &request.frame {
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
                Frame::Bulk(s) => command.push(String::from_utf8(s.to_vec()).map_err(|_| {
                    ServerError::CommandInvalidSyntax("must be utf8 string?".to_string())
                })?),
                _ => {
                    return Err(ServerError::CommandInvalidSyntax(
                        "shold be RESP array of bulk strings".to_string(),
                    ))
                }
            };
        }

        if command.is_empty() {
            return Err(ServerError::CommandInvalidSyntax(
                "missing command name".to_string(),
            ));
        }

        let command_name = command[0].to_lowercase();

        match command_name.as_str() {
            "echo" => echo::command(self, &request, &command).await,
            "ping" => ping::command(self, &request, &command).await,
            _ => return Err(ServerError::CommandNotAvailable(command_name)),
        };
        Ok(())
    }
}
