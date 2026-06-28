use tokio::{
    net::{TcpListener, TcpStream},
    select,
    sync::mpsc,
};

use crate::{
    messages::{ConnectionMessage, Request, ServerMessage},
    resp::{connection::Connection, error::FrameParsingError, types::Frame},
};

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
            Ok(Some((frame, _))) = connection.read::<Frame, FrameParsingError>() => {
                if let Err(e) = sender.send(ConnectionMessage::ClientRequest(Request {
                    client_id: id,
                    frame,
                    connection: connection_sender.clone()
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
