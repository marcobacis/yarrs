use std::time::Duration;

use redis::{aio::MultiplexedConnection, AsyncConnectionConfig, Value};
use yarrs::{
    listener::{bind, run_listener},
    server::Server,
};

#[tokio::test]
async fn test_ping() {
    let mut connection = spawn().await;
    let cmd = redis::Cmd::ping();
    let result = connection
        .send_packed_command(&cmd)
        .await
        .expect("Error sending ping command");

    assert_eq!(result, Value::BulkString("PONG".into()));
}

#[tokio::test]
async fn test_echo() {
    let mut connection = spawn().await;
    let mut cmd = redis::cmd("ECHO");
    cmd.arg("test string");

    let result = connection
        .send_packed_command(&cmd)
        .await
        .expect("Error sending echo command");

    assert_eq!(result, Value::BulkString("test string".into()));
}

async fn spawn() -> MultiplexedConnection {
    let mut listener = bind("0.0.0.0".into(), 0).await;
    let mut server = Server::new("0.0.0.0".into(), listener.local_addr().unwrap().port());
    let sender = server.sender.clone();
    let addr = server.info.address();

    tokio::spawn(async move {
        run_listener(&mut listener, sender).await;
    });

    tokio::spawn(async move {
        server.run().await;
    });

    let client =
        redis::Client::open(format!("redis://{}/", &addr)).expect("Could not create redis client");

    let config = AsyncConnectionConfig::new()
        .set_connection_timeout(Duration::from_secs(1))
        .set_response_timeout(Duration::from_secs(1));

    let connection = client
        .get_multiplexed_async_connection_with_config(&config)
        .await
        .expect("Could not connect to yarrs");

    connection
}
