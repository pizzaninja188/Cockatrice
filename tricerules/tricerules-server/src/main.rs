//! tricerules sidecar: TCP `127.0.0.1:TRICERULES_PORT` (default 17381).
//! Framing: u32 BE length + protobuf `IpcEnvelope` / `IpcResponse`.

use prost::Message;
use std::env;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tricerules_core::{GameEngine, PlayerId};
use tricerules_proto::ruled::v1::ipc_envelope::Msg;
use tricerules_proto::ruled::v1::{IpcEnvelope, IpcResponse};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let port: u16 = env::var("TRICERULES_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(17381);
    let addr = format!("127.0.0.1:{port}");
    let listener = TcpListener::bind(&addr).await?;
    eprintln!("tricerules-server listening on {addr}");
    loop {
        let (sock, _) = listener.accept().await?;
        tokio::spawn(async move {
            if let Err(e) = handle_connection(sock).await {
                eprintln!("connection error: {e}");
            }
        });
    }
}

async fn handle_connection(mut sock: TcpStream) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut engine: Option<GameEngine> = None;
    loop {
        let env = read_proto::<IpcEnvelope>(&mut sock).await?;
        let resp = match env.msg {
            Some(Msg::SessionStart(s)) => {
                let pids: Vec<PlayerId> = s.player_ids;
                match GameEngine::new(s.seed, &pids, 20) {
                    Ok(e) => {
                        let batch = e.initial_response_batch();
                        engine = Some(e);
                        IpcResponse {
                            ok: true,
                            error: String::new(),
                            batch: Some(batch),
                        }
                    }
                    Err(err) => IpcResponse {
                        ok: false,
                        error: err.to_string(),
                        batch: None,
                    },
                }
            }
            Some(Msg::PlayerCommand(pc)) => {
                if let Some(ref mut eng) = engine {
                    eng.player_command_ipc(pc.player_id, &pc.ruled_command)
                } else {
                    IpcResponse {
                        ok: false,
                        error: "no session".into(),
                        batch: None,
                    }
                }
            }
            Some(Msg::SessionEnd(_)) | None => {
                break;
            }
        };
        write_proto(&mut sock, &resp).await?;
    }
    Ok(())
}

async fn read_proto<M: Message + Default>(sock: &mut TcpStream) -> Result<M, Box<dyn std::error::Error + Send + Sync>> {
    let mut lenbuf = [0u8; 4];
    sock.read_exact(&mut lenbuf).await?;
    let len = u32::from_be_bytes(lenbuf) as usize;
    let mut buf = vec![0u8; len];
    sock.read_exact(&mut buf).await?;
    Ok(M::decode(&buf[..])?)
}

async fn write_proto<M: Message>(sock: &mut TcpStream, msg: &M) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let buf = msg.encode_to_vec();
    let len = (buf.len() as u32).to_be_bytes();
    sock.write_all(&len).await?;
    sock.write_all(&buf).await?;
    Ok(())
}
