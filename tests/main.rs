use std::time::Duration;

use redis::{aio::MultiplexedConnection, AsyncConnectionConfig, Value};
use tokio::net::TcpListener;

#[tokio::test]
async fn test_ping() {
    let mut connection = spawn().await;
    let cmd = redis::Cmd::ping();
    let result = connection
        .send_packed_command(&cmd)
        .await
        .expect("Error sending ping command");

    assert_eq!(result, Value::SimpleString("PONG".into()));
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

    assert_eq!(result, Value::SimpleString("test string".into()));
}

async fn spawn() -> MultiplexedConnection {
    let listener = TcpListener::bind("0.0.0.0:0")
        .await
        .expect("Couldn't create tcp listener");
    let addr = listener.local_addr().unwrap().to_string();

    tokio::spawn(async move {
        yarrs::run(listener).await.expect("Could not start yarrs");
    });

    let client =
        redis::Client::open(format!("redis://{}/", addr)).expect("Could not create redis client");

    let config = AsyncConnectionConfig::new()
        .set_connection_timeout(Duration::from_secs(1))
        .set_response_timeout(Duration::from_secs(1));

    let connection = client
        .get_multiplexed_async_connection_with_config(&config)
        .await
        .expect("Could not connect to yarrs");

    connection
}

/*
TEST LIST
- [X] Simple ping
- [ ] Ping two times
- [ ] ECHO (requires parsing?)
- [ ]
- [ ]
- [ ]
v

*/
