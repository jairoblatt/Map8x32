use dashmap::DashMap;
use std::io;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixListener;

type StorageType = Arc<DashMap<u8, Vec<u32>>>;

const OP_SET: u8 = 1;
const OP_GET: u8 = 2;
const OP_DELETE_BY_KEY: u8 = 3;
const OP_DELETE_ALL: u8 = 4;
const OP_LIST_ALL: u8 = 5;

const STATUS_NOT_FOUND: u8 = 0;
const STATUS_OK: u8 = 1;
const STATUS_BAD_REQUEST: u8 = 2;


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
                    OP_SET => {
                        storage_clone
                            .entry(key)
                            .or_insert_with(Vec::new)
                            .push(value);

                        if socket.write_u8(STATUS_OK).await.is_err() {
                            break;
                        }
                    }
                    OP_GET => {
                        if let Some(values) = storage_clone.get(&key) {
                            if socket.write_u8(STATUS_OK).await.is_err() {
                                break;
                            }

                            if socket.write_u32_le(values.len() as u32).await.is_err() {
                                break;
                            }

                            let mut write_failed = false;

                            for &v in values.iter() {
                                if socket.write_u32_le(v).await.is_err() {
                                    write_failed = true;
                                    break;
                                }
                            }

                            if write_failed {
                                break;
                            }
                        } else {
                            if socket.write_u8(STATUS_NOT_FOUND).await.is_err() {
                                break;
                            }
                        }
                    }
                    OP_DELETE_BY_KEY => {

                        if let Some(_) = storage_clone.get(&key) {

                            storage_clone.remove(&key);
                            if socket.write_u8(STATUS_OK).await.is_err() {
                                break;
                            }
                        } else {
                            if socket.write_u8(STATUS_NOT_FOUND).await.is_err() {
                                break;
                            }
                        }
                    }
                    OP_DELETE_ALL => {
                        storage_clone.clear();
                        if socket.write_u8(STATUS_OK).await.is_err() {
                            break;
                        }
                    }
                    OP_LIST_ALL => {
                        if socket.write_u8(STATUS_OK).await.is_err() {
                            break;
                        }

                        if socket.write_u32_le(storage_clone.len() as u32).await.is_err() {
                            break;
                        }

                        let mut write_failed = false;

                        for entry in storage_clone.iter() {
                            let key = *entry.key();
                            let values = entry.value();

                            if socket.write_u8(key).await.is_err() {
                                write_failed = true;
                                break;
                            }

                            if socket.write_u32_le(values.len() as u32).await.is_err() {
                                write_failed = true;
                                break;
                            }

                            for &v in values.iter() {
                                if socket.write_u32_le(v).await.is_err() {
                                    write_failed = true;
                                    break;
                                }
                            }

                            if write_failed {
                                break;
                            }
                        }

                        if write_failed {
                            break;
                        }
                    }
                    _ => {
                        if socket.write_u8(STATUS_BAD_REQUEST).await.is_err() {
                            break;
                        }
                    }
                }
            }
        });
    }
}
