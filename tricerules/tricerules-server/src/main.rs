//! tricerules sidecar: TCP `127.0.0.1:TRICERULES_PORT` (default 17381).
//! Framing: u32 BE length + protobuf `IpcEnvelope` / `IpcResponse`.

use prost::Message;
use std::collections::BTreeSet;
use std::env;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tricerules_cards::CardRegistry;
use tricerules_core::{GameEngine, PlayerId};
use tricerules_proto::ruled::v1::ipc_envelope::Msg;
use tricerules_proto::ruled::v1::{IpcEnvelope, IpcResponse, PlayerDeck};

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

async fn handle_connection(
    mut sock: TcpStream,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut engine: Option<GameEngine> = None;
    loop {
        let env = read_proto::<IpcEnvelope>(&mut sock).await?;
        let resp = match env.msg {
            Some(Msg::SessionStart(s)) => {
                let pids: Vec<PlayerId> = s.player_ids;
                match resolve_deck_names(&pids, &s.player_decks) {
                    Err(missing) => IpcResponse {
                        ok: false,
                        error: format!("unimplemented cards: {}", missing.join(", ")),
                        batch: None,
                    },
                    Ok(decks) => match GameEngine::new(s.seed, &pids, 20, decks, false) {
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

/// Resolves per-player decks of Oracle card names into engine card ids via the shared
/// registry (the engine owns card identity; Servatrice never derives ids). Returns the
/// per-player id lists aligned with `pids` (`None` when no decks were supplied), or the
/// sorted, deduplicated list of every name the engine does not implement.
fn resolve_deck_names(
    pids: &[PlayerId],
    player_decks: &[PlayerDeck],
) -> Result<Option<Vec<Vec<String>>>, Vec<String>> {
    if player_decks.is_empty() {
        return Ok(None);
    }
    let registry = CardRegistry::global();
    let mut out: Vec<Vec<String>> = (0..pids.len()).map(|_| vec![]).collect();
    let mut missing: BTreeSet<String> = BTreeSet::new();
    for pd in player_decks {
        let Some(i) = pids.iter().position(|&x| x == pd.player_id) else {
            continue;
        };
        for name in &pd.mainboard_card_name {
            match registry.id_for_name(name) {
                Some(id) => out[i].push(id.to_string()),
                None => {
                    missing.insert(name.trim().to_string());
                }
            }
        }
    }
    if missing.is_empty() {
        Ok(Some(out))
    } else {
        Err(missing.into_iter().collect())
    }
}

async fn read_proto<M: Message + Default>(
    sock: &mut TcpStream,
) -> Result<M, Box<dyn std::error::Error + Send + Sync>> {
    let mut lenbuf = [0u8; 4];
    sock.read_exact(&mut lenbuf).await?;
    let len = u32::from_be_bytes(lenbuf) as usize;
    let mut buf = vec![0u8; len];
    sock.read_exact(&mut buf).await?;
    Ok(M::decode(&buf[..])?)
}

async fn write_proto<M: Message>(
    sock: &mut TcpStream,
    msg: &M,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let buf = msg.encode_to_vec();
    if buf.len() > 2000 {
        eprintln!(
            "tricerules: IpcResponse {} bytes (SessionStart; ensure servatrice and this sidecar are rebuilt from the same tree)",
            buf.len()
        );
    }
    let len = (buf.len() as u32).to_be_bytes();
    sock.write_all(&len).await?;
    sock.write_all(&buf).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn deck(player_id: PlayerId, names: &[&str]) -> PlayerDeck {
        PlayerDeck {
            player_id,
            mainboard_card_name: names.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn resolve_deck_names_maps_oracle_names_to_ids() {
        let decks = resolve_deck_names(
            &[0, 1],
            &[
                deck(0, &["Lightning Bolt", " mountain "]),
                deck(1, &["Forest"]),
            ],
        )
        .expect("all names implemented")
        .expect("decks supplied");
        assert_eq!(decks[0], vec!["lightning_bolt", "mountain"]);
        assert_eq!(decks[1], vec!["forest"]);
    }

    #[test]
    fn resolve_deck_names_collects_all_missing_sorted_deduped() {
        let err = resolve_deck_names(
            &[0, 1],
            &[
                deck(0, &["Black Lotus", "Mountain", "Brainstorm"]),
                deck(1, &["Brainstorm", "Forest"]),
            ],
        )
        .expect_err("unimplemented names must fail the session");
        assert_eq!(err, vec!["Black Lotus", "Brainstorm"]);
    }

    #[test]
    fn resolve_deck_names_empty_means_engine_default_decks() {
        assert!(matches!(resolve_deck_names(&[0, 1], &[]), Ok(None)));
    }
}
