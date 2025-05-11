use redis::{aio::MultiplexedConnection, AsyncCommands};
use tokio::net::TcpListener;

#[tokio::test]
async fn test_ping() {
    let mut connection = spawn().await;
    let result = connection.ping::<String>().await;
    assert!(result.is_ok());
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
    client
        .get_multiplexed_async_connection()
        .await
        .expect("could not connect to server")
}

/*
TEST LIST
- [X] Simple ping
- [X] Ping two times
- [ ] ECHO (requires parsing?)
- [ ]
- [ ]
- [ ]
v

*/
