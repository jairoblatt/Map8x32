use dashmap::DashMap;
use std::io;
use std::sync::Arc;
use tokio::fs;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixListener;

type StorageType = Arc<DashMap<u8, Vec<u32>>>;

#[tokio::main]
async fn main() -> io::Result<()> {
    let storage: StorageType = Arc::new(DashMap::new());

    let addr = "/tmp/map8x32.sock";

    if tokio::fs::try_exists(addr).await.unwrap_or(false) {
        tokio::fs::remove_file(addr).await?;
    }

    let listener = UnixListener::bind(addr)?;

    loop {
        let (mut socket, _) = listener.accept().await?;
        let storage_clone = storage.clone();

        tokio::task::spawn(async move {
            let mut buf = [0u8; 6];

            while let Ok(_) = socket.read_exact(&mut buf).await {
                let op = buf[0];
                let key = buf[1];
                let value = u32::from_le_bytes([buf[2], buf[3], buf[4], buf[5]]);

                match op {
                    // SET
                    1 => {
                        storage_clone
                            .entry(key)
                            .or_insert_with(Vec::new)
                            .push(value);
                        //OK
                        let _ = socket.write_u8(1).await;
                    }
                    // GET
                    2 => {
                        if let Some(values) = storage_clone.get(&key) {
                            // OK
                            let _ = socket.write_u8(1).await;
                            let _ = socket.write_u32_le(values.len() as u32).await;
                            for &v in values.iter() {
                                let _ = socket.write_u32_le(v).await;
                            }
                        } else {
                            // NOT_FOUND
                            let _ = socket.write_u8(0).await;
                        }
                    }
                    _ => {
                        // BAD_REQUEST
                        let _ = socket.write_u8(2).await;
                    }
                }
            }
        });
    }
}
