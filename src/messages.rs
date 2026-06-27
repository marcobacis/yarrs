use tokio::sync::mpsc;

use crate::resp::types::Frame;

pub enum ConnectionMessage {
    NewClient(mpsc::Sender<ServerMessage>),
    ClientRequest(Request),
}

pub enum ServerMessage {
    ClientInitialized(u64),
    Data(Frame),
}

pub struct Request {
    pub client_id: u64,
    pub frame: Frame,
}
