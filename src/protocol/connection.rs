use bytes::Buf;
use bytes::BytesMut;
use std::io::Cursor;
use std::io::ErrorKind;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;

pub trait Message<T, TErr> {
    fn check(cursor: &mut Cursor<&[u8]>) -> bool;
    fn parse(cursor: &mut Cursor<&[u8]>) -> Result<T, TErr>;
}

pub struct Connection<T>
where
    T: AsyncReadExt + AsyncWriteExt + Unpin,
{
    stream: T,
    buffer: BytesMut,
}

impl<T> Connection<T>
where
    T: AsyncReadExt + AsyncWriteExt + Unpin,
{
    pub fn new(stream: T) -> Self {
        Self {
            stream,
            buffer: BytesMut::with_capacity(4096),
        }
    }

    pub async fn read<TItem, TErr>(&mut self) -> Result<Option<TItem>, TErr>
    where
        TItem: Message<TItem, TErr>,
        TErr: From<std::io::Error>,
    {
        loop {
            let mut cursor = Cursor::new(&self.buffer[..]);
            if TItem::check(&mut cursor) {
                cursor.set_position(0);
                let result = match TItem::parse(&mut cursor) {
                    Ok(msg) => Ok(Some(msg)),
                    Err(e) => Err(e),
                };
                self.buffer.advance(cursor.position() as usize);
                return result;
            }

            if 0 == self.stream.read_buf(&mut self.buffer).await? {
                if !self.buffer.is_empty() {
                    return Err(
                        std::io::Error::new(ErrorKind::BrokenPipe, "Connection closed").into(),
                    );
                }
                return Ok(None);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use tokio::io::AsyncWriteExt;

    use super::Message;
    use crate::protocol::connection::Connection;

    #[tokio::test]
    async fn can_create_connection() {
        let (_, server) = tokio::io::duplex(64);
        let mut _connection = Connection::new(server);
    }

    #[tokio::test]
    async fn connection_closed_returns_error() {
        let (mut client, server) = tokio::io::duplex(64);
        let mut connection = Connection::new(server);

        client.write_all(&[1, 2, 3]).await.unwrap();
        drop(client);

        let result = connection.read::<TestCoso<10>, anyhow::Error>().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn clean_closed_connection_returns_none() {
        let (client, server) = tokio::io::duplex(64);
        let mut connection = Connection::new(server);

        drop(client);
        let result = connection.read::<TestCoso<10>, anyhow::Error>().await;

        assert!(result.is_ok_and(|r| r.is_none()));
    }

    #[tokio::test]
    async fn read_complete_message() {
        let (mut client, server) = tokio::io::duplex(64);
        let mut connection = Connection::new(server);

        client
            .write_all(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10])
            .await
            .unwrap();
        let result = connection.read::<TestCoso<10>, anyhow::Error>().await;

        assert_msg_is([1, 2, 3, 4, 5, 6, 7, 8, 9, 10], result);
    }

    #[tokio::test]
    async fn read_two_complete_messages() {
        let (mut client, server) = tokio::io::duplex(64);
        let mut connection = Connection::new(server);

        client
            .write_all(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10])
            .await
            .unwrap();

        let first = connection.read::<TestCoso<5>, anyhow::Error>().await;
        let second = connection.read::<TestCoso<5>, anyhow::Error>().await;

        assert_msg_is([1, 2, 3, 4, 5], first);
        assert_msg_is([6, 7, 8, 9, 10], second);
    }

    fn assert_msg_is<const N: usize, TErr>(
        expected: [u8; N],
        result: Result<Option<TestCoso<N>>, TErr>,
    ) {
        assert!(result.is_ok_and(|r| r.is_some_and(|msg| {
            dbg!(&msg);
            msg.buf == expected
        })));
    }

    // Test implementation of a message, requiring a fixed number of bytes per message (N)
    #[derive(Debug)]
    struct TestCoso<const N: usize> {
        buf: [u8; N],
    }

    impl<const N: usize> Message<TestCoso<N>, anyhow::Error> for TestCoso<N> {
        fn check(cursor: &mut std::io::Cursor<&[u8]>) -> bool {
            let mut buf = [0u8; N];
            let bytes_read = match std::io::Read::read(cursor, &mut buf) {
                Ok(n) => {
                    dbg!(n);
                    n
                }
                Err(_) => return false,
            };
            bytes_read >= N
        }

        fn parse(cursor: &mut std::io::Cursor<&[u8]>) -> Result<TestCoso<N>, anyhow::Error> {
            let mut msg = Self { buf: [0u8; N] };
            let _ = std::io::Read::read(cursor, &mut msg.buf)?;
            Ok(msg)
        }
    }
}
