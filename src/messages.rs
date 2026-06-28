use tokio::sync::mpsc;

use crate::{resp::types::Frame, server::ServerError};

#[derive(Debug)]
pub enum ConnectionMessage {
    NewClient(mpsc::Sender<ServerMessage>),
    ClientRequest(Request),
}

#[derive(Debug, PartialEq)]
pub enum ServerMessage {
    ClientInitialized(u64),
    Data(Frame),
    Error(ServerError),
}

#[derive(Debug)]
pub struct Request {
    pub client_id: u64,
    pub frame: Frame,
    pub connection: mpsc::Sender<ServerMessage>,
}

impl Request {
    pub async fn data(&self, frame: Frame) {
        self.connection
            .send(ServerMessage::Data(frame))
            .await
            .unwrap();
    }

    pub async fn error(&self, error: ServerError) {
        self.connection
            .send(ServerMessage::Error(error))
            .await
            .unwrap();
    }
}
