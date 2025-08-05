use dashmap::DashMap;
use std::io;
use std::sync::Arc;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UnixListener;
use tokio::sync::{mpsc, oneshot};
use std::os::unix::fs::PermissionsExt;

type StorageType = Arc<DashMap<u8, Vec<u32>>>;

const OP_SET: u8 = 1;
const OP_GET: u8 = 2;
const OP_DELETE_BY_KEY: u8 = 3;
const OP_DELETE_ALL: u8 = 4;
const OP_LIST_ALL: u8 = 5;

const STATUS_NOT_FOUND: u8 = 0;
const STATUS_OK: u8 = 1;
const STATUS_BAD_REQUEST: u8 = 2;

#[derive(Debug)]
enum Command {
    Set { key: u8, value: u32, respond_to: oneshot::Sender<u8> },
    Get { key: u8, respond_to: oneshot::Sender<GetResponse> },
    DeleteByKey { key: u8, respond_to: oneshot::Sender<u8> },
    DeleteAll { respond_to: oneshot::Sender<u8> },
    ListAll { respond_to: oneshot::Sender<ListAllResponse> },
}

#[derive(Debug)]
enum GetResponse {
    Found(Vec<u32>),
    NotFound,
}

#[derive(Debug)]
struct ListAllResponse {
    entries: Vec<(u8, Vec<u32>)>,
}

async fn command_processor(mut receiver: mpsc::UnboundedReceiver<Command>, storage: StorageType) {
    while let Some(command) = receiver.recv().await {
        match command {
            Command::Set { key, value, respond_to } => {
                storage.entry(key).or_insert_with(Vec::new).push(value);
                let _ = respond_to.send(STATUS_OK);
            }
            Command::Get { key, respond_to } => {
                let response = if let Some(values) = storage.get(&key) {
                    GetResponse::Found(values.clone())
                } else {
                    GetResponse::NotFound
                };
                let _ = respond_to.send(response);
            }
            Command::DeleteByKey { key, respond_to } => {
                let status = if storage.remove(&key).is_some() {
                    STATUS_OK
                } else {
                    STATUS_NOT_FOUND
                };
                let _ = respond_to.send(status);
            }
            Command::DeleteAll { respond_to } => {
                storage.clear();
                let _ = respond_to.send(STATUS_OK);
            }
            Command::ListAll { respond_to } => {
                let entries: Vec<(u8, Vec<u32>)> = storage
                    .iter()
                    .map(|entry| (*entry.key(), entry.value().clone()))
                    .collect();
                let _ = respond_to.send(ListAllResponse { entries });
            }
        }
    }
}

async fn handle_connection(
    mut socket: tokio::net::UnixStream,
    sender: mpsc::UnboundedSender<Command>,
) {
    let mut buf = [0u8; 6];

    while let Ok(_) = socket.read_exact(&mut buf).await {
        let op = buf[0];
        let key = buf[1];
        let value = u32::from_le_bytes([buf[2], buf[3], buf[4], buf[5]]);

        match op {
            OP_SET => {
                let (tx, rx) = oneshot::channel();
                if sender.send(Command::Set { key, value, respond_to: tx }).is_err() {
                    break;
                }
                if let Ok(status) = rx.await {
                    if socket.write_u8(status).await.is_err() {
                        break;
                    }
                } else {
                    break;
                }
            }
            OP_GET => {
                let (tx, rx) = oneshot::channel();
                if sender.send(Command::Get { key, respond_to: tx }).is_err() {
                    break;
                }
                if let Ok(response) = rx.await {
                    match response {
                        GetResponse::Found(values) => {
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
                        }
                        GetResponse::NotFound => {
                            if socket.write_u8(STATUS_NOT_FOUND).await.is_err() {
                                break;
                            }
                        }
                    }
                } else {
                    break;
                }
            }
            OP_DELETE_BY_KEY => {
                let (tx, rx) = oneshot::channel();
                if sender.send(Command::DeleteByKey { key, respond_to: tx }).is_err() {
                    break;
                }
                if let Ok(status) = rx.await {
                    if socket.write_u8(status).await.is_err() {
                        break;
                    }
                } else {
                    break;
                }
            }
            OP_DELETE_ALL => {
                let (tx, rx) = oneshot::channel();
                if sender.send(Command::DeleteAll { respond_to: tx }).is_err() {
                    break;
                }
                if let Ok(status) = rx.await {
                    if socket.write_u8(status).await.is_err() {
                        break;
                    }
                } else {
                    break;
                }
            }
            OP_LIST_ALL => {
                let (tx, rx) = oneshot::channel();
                if sender.send(Command::ListAll { respond_to: tx }).is_err() {
                    break;
                }
                if let Ok(response) = rx.await {
                    if socket.write_u8(STATUS_OK).await.is_err() {
                        break;
                    }
                    if socket.write_u32_le(response.entries.len() as u32).await.is_err() {
                        break;
                    }
                    let mut write_failed = false;
                    for (key, values) in response.entries {
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
                } else {
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
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> io::Result<()> {
    let storage: StorageType = Arc::new(DashMap::new());
    let (sender, receiver) = mpsc::unbounded_channel();

    tokio::spawn(command_processor(receiver, storage.clone()));

    let addr = "/tmp/map8x32.sock";

    if tokio::fs::try_exists(addr).await.unwrap_or(false) {
        tokio::fs::remove_file(addr).await?;
    }

    let listener = UnixListener::bind(addr)?;
    
    let mut perms = tokio::fs::metadata(addr).await?.permissions();
    perms.set_mode(0o666);
    tokio::fs::set_permissions(addr, perms).await?;

    loop {
        let (socket, _) = listener.accept().await?;
        let sender_clone = sender.clone();

        tokio::spawn(handle_connection(socket, sender_clone));
    }
}