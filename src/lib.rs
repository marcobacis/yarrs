use anyhow::Result;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
pub mod protocol;

pub async fn run(listener: TcpListener) -> Result<()> {
    println!("Starting server, listening for connections");
    loop {
        let (mut socket, _) = listener.accept().await?;
        tokio::spawn(async move {
            handle(&mut socket)
                .await
                .expect("Error handling connection");
        });
    }
}

async fn handle(socket: &mut TcpStream) -> Result<()> {
    let mut buffer = [0u8; 512];
    while let Ok(bytes_read) = socket.read(&mut buffer).await {
        if bytes_read == 0 {
            return Ok(());
        }
        let received = String::from_utf8(buffer.to_vec())?;
        if received.contains("PING") {
            socket.write("+PONG\r\n".as_bytes()).await?;
        }
    }
    Ok(())
}
