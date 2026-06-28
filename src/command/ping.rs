use crate::{messages::Request, resp::types::Frame, server::Server};

pub async fn command(_server: &Server, request: &Request, command: &Vec<String>) {
    if command.len() > 1 {
        request
            .data(Frame::Bulk(command[1].as_bytes().to_vec().into()))
            .await;
        return;
    }

    request.data(Frame::Bulk("PONG".into())).await;
}

#[cfg(test)]
mod tests {
    use tokio::sync::mpsc;

    use crate::{
        command::ping::command,
        messages::{Request, ServerMessage},
        resp::types::Frame,
        server::Server,
    };

    #[tokio::test]
    async fn test_ping_no_argument() {
        let (server, mut connection_receiver, request, cmd) =
            setup_command_test(vec!["ping".into()]);

        command(&server, &request, &cmd).await;

        assert_eq!(
            connection_receiver.try_recv().unwrap(),
            ServerMessage::Data(Frame::Bulk("PONG".into()))
        );
    }

    #[tokio::test]
    async fn test_ping_argument() {
        let (server, mut connection_receiver, request, cmd) =
            setup_command_test(vec!["ping".into(), "argument".into()]);

        command(&server, &request, &cmd).await;

        assert_eq!(
            connection_receiver.try_recv().unwrap(),
            ServerMessage::Data(Frame::Bulk("argument".into()))
        );
    }

    fn setup_command_test(
        cmd: Vec<String>,
    ) -> (Server, mpsc::Receiver<ServerMessage>, Request, Vec<String>) {
        let server = Server::new("0.0.0.0".into(), 0);
        let (connection_sender, connection_receiver) = mpsc::channel::<ServerMessage>(32);
        let cmd_frames: Vec<Frame> = cmd
            .iter()
            .map(|s| Frame::Bulk(s.as_bytes().to_vec().into()))
            .collect();
        let request = Request {
            client_id: 0,
            frame: Frame::Array(cmd_frames),
            connection: connection_sender.clone(),
        };

        (server, connection_receiver, request, cmd.clone())
    }
}
