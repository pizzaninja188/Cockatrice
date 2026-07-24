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

/// This sidecar's build id, reported to Servatrice in the SessionStart handshake.
const ENGINE_BUILD: &str = env!("CARGO_PKG_VERSION");

/// Failure response shared by ValidateDeck and SessionStart name resolution:
/// `missing` must be the sorted, deduplicated unimplemented Oracle names.
fn missing_cards_response(missing: Vec<String>) -> IpcResponse {
    IpcResponse {
        ok: false,
        error: format!("unimplemented cards: {}", missing.join(", ")),
        batch: None,
        missing_card_names: missing,
        engine_build: String::new(),
        card_data_hash: String::new(),
    }
}

/// Stateless ValidateDeck: no engine, no session — pure registry lookups.
/// ok iff every name resolves to an implemented card id.
fn validate_deck_response(card_names: &[String]) -> IpcResponse {
    let registry = CardRegistry::global();
    let missing: BTreeSet<String> = card_names
        .iter()
        .filter(|name| registry.id_for_name(name).is_none())
        .map(|name| name.trim().to_string())
        .collect();
    if missing.is_empty() {
        IpcResponse {
            ok: true,
            error: String::new(),
            batch: None,
            missing_card_names: vec![],
            engine_build: String::new(),
            card_data_hash: String::new(),
        }
    } else {
        missing_cards_response(missing.into_iter().collect())
    }
}

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
                // Server-side only (never broadcast to clients): the seed here plus the logged
                // command stream is what reproduces a session, and the E2E smoke test asserts
                // its forced seed reached the engine through this line.
                eprintln!(
                    "tricerules: session start game {} seed {} (servatrice build {})",
                    s.game_id,
                    s.seed,
                    if s.servatrice_build.is_empty() {
                        "unknown"
                    } else {
                        s.servatrice_build.as_str()
                    }
                );
                let pids: Vec<PlayerId> = s.player_ids;
                match resolve_deck_names(&pids, &s.player_decks) {
                    Err(missing) => missing_cards_response(missing),
                    Ok(decks) => match GameEngine::new(s.seed, &pids, 20, decks, false) {
                        Ok(e) => {
                            let batch = e.initial_response_batch();
                            engine = Some(e);
                            // Version handshake: stamp the sidecar build + card-data hash so
                            // Servatrice can log skew and record the hash in the replay.
                            IpcResponse {
                                ok: true,
                                error: String::new(),
                                batch: Some(batch),
                                missing_card_names: vec![],
                                engine_build: ENGINE_BUILD.to_string(),
                                card_data_hash: CardRegistry::content_hash(),
                            }
                        }
                        Err(err) => IpcResponse {
                            ok: false,
                            error: err.to_string(),
                            batch: None,
                            missing_card_names: vec![],
                            engine_build: String::new(),
                            card_data_hash: String::new(),
                        },
                    },
                }
            }
            Some(Msg::ValidateDeck(v)) => validate_deck_response(&v.card_names),
            Some(Msg::PlayerCommand(pc)) => {
                if let Some(ref mut eng) = engine {
                    eng.player_command_ipc(pc.player_id, &pc.ruled_command)
                } else {
                    IpcResponse {
                        ok: false,
                        error: "no session".into(),
                        batch: None,
                        missing_card_names: vec![],
                        engine_build: String::new(),
                        card_data_hash: String::new(),
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
    // Cap the framed length so a corrupt/oversized prefix can't trigger a multi-GB allocation
    // (mirrors the 16 MiB cap on the Servatrice relay side; full zone sync is a few KB).
    const MAX_FRAME_LEN: usize = 16 * 1024 * 1024;
    if len > MAX_FRAME_LEN {
        return Err(format!("frame length {len} exceeds cap {MAX_FRAME_LEN}").into());
    }
    let mut buf = vec![0u8; len];
    sock.read_exact(&mut buf).await?;
    Ok(M::decode(&buf[..])?)
}

async fn write_proto<M: Message>(
    sock: &mut TcpStream,
    msg: &M,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let buf = msg.encode_to_vec();
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
                deck(0, &["Black Lotus", "Mountain", "Time Walk"]),
                deck(1, &["Time Walk", "Forest"]),
            ],
        )
        .expect_err("unimplemented names must fail the session");
        assert_eq!(err, vec!["Black Lotus", "Time Walk"]);
    }

    #[test]
    fn resolve_deck_names_empty_means_engine_default_decks() {
        assert!(matches!(resolve_deck_names(&[0, 1], &[]), Ok(None)));
    }

    fn names(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn validate_deck_all_implemented_is_ok() {
        let resp = validate_deck_response(&names(&["Lightning Bolt", " mountain ", "Forest"]));
        assert!(resp.ok);
        assert!(resp.error.is_empty());
        assert!(resp.missing_card_names.is_empty());
        assert!(resp.batch.is_none());
    }

    #[test]
    fn validate_deck_reports_missing_sorted_deduped() {
        let resp = validate_deck_response(&names(&[
            "Black Lotus",
            "Mountain",
            "Time Walk",
            "Time Walk",
            " black lotus ",
        ]));
        assert!(!resp.ok);
        // Reported strings are the trimmed raw spellings, so the case-variant
        // " black lotus " stays a distinct entry alongside "Black Lotus".
        assert_eq!(
            resp.missing_card_names,
            vec!["Black Lotus", "Time Walk", "black lotus"]
        );
        assert_eq!(
            resp.error,
            "unimplemented cards: Black Lotus, Time Walk, black lotus"
        );
        assert!(resp.batch.is_none());
    }

    #[test]
    fn validate_deck_empty_list_is_ok() {
        let resp = validate_deck_response(&[]);
        assert!(resp.ok);
        assert!(resp.missing_card_names.is_empty());
    }

    #[test]
    fn session_start_missing_fills_missing_card_names() {
        let missing = resolve_deck_names(&[0], &[deck(0, &["Black Lotus", "Mountain"])])
            .expect_err("Black Lotus is not implemented");
        let resp = missing_cards_response(missing);
        assert!(!resp.ok);
        assert_eq!(resp.missing_card_names, vec!["Black Lotus"]);
        assert_eq!(resp.error, "unimplemented cards: Black Lotus");
    }
}
