//! tricerules sidecar: TCP `127.0.0.1:TRICERULES_PORT` (default 17381).
//! Framing: u32 BE length + protobuf `IpcEnvelope` / `IpcResponse`.

use prost::Message;
use std::collections::BTreeSet;
use std::env;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tricerules_cards::CardRegistry;
use tricerules_core::{GameEngine, PlayerId};
use tricerules_proto::ruled::v1::ipc_envelope::Msg;
use tricerules_proto::ruled::v1::{IpcEnvelope, IpcResponse, PlayerDeck};

/// This sidecar's build id, reported to Servatrice in the SessionStart handshake.
const ENGINE_BUILD: &str = env!("CARGO_PKG_VERSION");

/// Whether this sidecar process was started with cheat commands permitted.
/// Split from the env read so the gate is testable without mutating process environment.
fn dev_commands_allowed(env_value: Option<&str>) -> bool {
    matches!(env_value, Some("1") | Some("true"))
}

/// The dev gate, both halves (see `SessionStart.dev_commands_enabled` and `DevCommand`).
///
/// Servatrice must ask for dev commands *and* this sidecar process must have been started with
/// `TRICERULES_DEV_COMMANDS` set. Requiring both means a production sidecar cannot be talked into
/// dev mode from upstream: enabling cheats takes access to the machine running the engine.
fn dev_commands_enabled_for_session(requested: bool, env_value: Option<&str>) -> bool {
    requested && dev_commands_allowed(env_value)
}

/// Default idle timeout once a session exists: 4 hours without a single byte from the peer.
///
/// Deliberately far above human turn time. Servatrice keeps **one connection per ruled game**, open
/// for the whole game and idle between commands for as long as players take to act, and dropping it
/// kills that game for good — the engine state cannot be recovered. This guard exists to bound a
/// *half-open* connection (a peer that died without closing its socket), not to police slow play, so
/// it is set where no plausible game reaches it.
const DEFAULT_SESSION_IDLE_TIMEOUT_SECS: u64 = 14400;

/// Default idle timeout before `SessionStart`: 60 seconds.
///
/// A connection with no session yet holds no game, so dropping it costs nothing. Servatrice sends
/// `SessionStart` (or `ValidateDeck`) within one round trip of connecting — no connection sits open
/// while players pick decks, since `validateDecksForStart` uses a short-lived stack-local relay and
/// the game's own relay is created and started together — so a minute is already orders of
/// magnitude of headroom.
const DEFAULT_PRE_SESSION_IDLE_TIMEOUT_SECS: u64 = 60;

/// How long a connection may go without the peer sending anything, before and after a session
/// exists. `None` disables the guard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct IdleTimeouts {
    pre_session: Option<Duration>,
    session: Option<Duration>,
}

impl IdleTimeouts {
    /// The timeout that applies right now. Splitting the two is the point: a leaked connection that
    /// never became a game dies in a minute, while a live game gets the long leash.
    fn for_session(&self, has_session: bool) -> Option<Duration> {
        if has_session {
            self.session
        } else {
            self.pre_session
        }
    }
}

/// `TRICERULES_IDLE_TIMEOUT_SECS` sets the **session** timeout; the pre-session one is capped at the
/// smaller of its default and that value, so a small test value shortens both and stays intuitive.
/// `0` disables both; an unparseable value falls back to the defaults rather than failing the process.
///
/// Split from the env read so the policy is testable without mutating process environment
/// (same reason as `dev_commands_allowed`).
fn idle_timeouts_from_env(env_value: Option<&str>) -> IdleTimeouts {
    let session_secs = env_value
        .and_then(|s| s.trim().parse::<u64>().ok())
        .unwrap_or(DEFAULT_SESSION_IDLE_TIMEOUT_SECS);
    let pre_session_secs = session_secs.min(DEFAULT_PRE_SESSION_IDLE_TIMEOUT_SECS);
    IdleTimeouts {
        pre_session: (pre_session_secs > 0).then(|| Duration::from_secs(pre_session_secs)),
        session: (session_secs > 0).then(|| Duration::from_secs(session_secs)),
    }
}

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

/// Decrements the live-session count on every exit path of a connection task (EOF, error,
/// idle timeout), so the shutdown log cannot drift from reality.
struct SessionGuard(Arc<AtomicUsize>);

impl Drop for SessionGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}

/// Resolves when the process is asked to terminate: Ctrl+C everywhere, plus SIGTERM on unix
/// (how a container or service manager stops us).
async fn shutdown_signal() {
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut term = match signal(SignalKind::terminate()) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("tricerules: cannot install SIGTERM handler ({e}), Ctrl+C only");
                let _ = tokio::signal::ctrl_c().await;
                return;
            }
        };
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {}
            _ = term.recv() => {}
        }
    }
    #[cfg(not(unix))]
    {
        let _ = tokio::signal::ctrl_c().await;
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let port: u16 = env::var("TRICERULES_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(17381);
    let idle_timeouts =
        idle_timeouts_from_env(env::var("TRICERULES_IDLE_TIMEOUT_SECS").ok().as_deref());
    let addr = format!("127.0.0.1:{port}");
    let listener = TcpListener::bind(&addr).await?;
    let describe = |t: Option<Duration>| match t {
        Some(d) => format!("{}s", d.as_secs()),
        None => "disabled".to_string(),
    };
    eprintln!(
        "tricerules-server listening on {addr} (idle timeout {} pre-session, {} in session)",
        describe(idle_timeouts.pre_session),
        describe(idle_timeouts.session)
    );
    let live_sessions = Arc::new(AtomicUsize::new(0));
    loop {
        let (sock, _) = tokio::select! {
            accepted = listener.accept() => accepted?,
            _ = shutdown_signal() => {
                eprintln!(
                    "tricerules: shutdown signal, {} live session(s), exiting",
                    live_sessions.load(Ordering::Relaxed)
                );
                return Ok(());
            }
        };
        // Framing is one write per message, but the relay is strictly request/response: never let
        // Nagle hold a response waiting for a follow-up that only arrives after the peer replies.
        if let Err(e) = sock.set_nodelay(true) {
            eprintln!("tricerules: could not set TCP_NODELAY ({e}), continuing");
        }
        live_sessions.fetch_add(1, Ordering::Relaxed);
        let guard = SessionGuard(Arc::clone(&live_sessions));
        tokio::spawn(async move {
            let _guard = guard;
            if let Err(e) = handle_connection(sock, idle_timeouts).await {
                eprintln!("connection error: {e}");
            }
        });
    }
}

async fn handle_connection(
    mut sock: TcpStream,
    idle_timeouts: IdleTimeouts,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut engine: Option<GameEngine> = None;
    let mut game_id: u64 = 0;
    loop {
        // Re-read per iteration: the applicable timeout lengthens the moment a session exists.
        let idle_timeout = idle_timeouts.for_session(engine.is_some());
        let env = match read_envelope(&mut sock, idle_timeout).await? {
            Some(env) => env,
            // Idle drop is expected housekeeping, not an error: returning Ok here keeps it out of
            // the `connection error:` log, and dropping the frame frees `engine` with its session.
            None => {
                eprintln!(
                    "tricerules: connection idle for {}s, dropping session (game {})",
                    idle_timeout.map(|d| d.as_secs()).unwrap_or(0),
                    if engine.is_some() {
                        game_id.to_string()
                    } else {
                        "none".to_string()
                    }
                );
                return Ok(());
            }
        };
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
                game_id = s.game_id;
                let pids: Vec<PlayerId> = s.player_ids;
                match resolve_deck_names(&pids, &s.player_decks) {
                    Err(missing) => missing_cards_response(missing),
                    Ok(decks) => match GameEngine::new(s.seed, &pids, 20, decks, false) {
                        Ok(mut e) => {
                            let dev_env = env::var("TRICERULES_DEV_COMMANDS").ok();
                            if dev_commands_enabled_for_session(
                                s.dev_commands_enabled,
                                dev_env.as_deref(),
                            ) {
                                eprintln!(
                                    "tricerules: DEV COMMANDS ENABLED for game {} — cheat commands are accepted",
                                    s.game_id
                                );
                                e.enable_dev_commands();
                            }
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
            Some(Msg::SpellPaymentQuery(query)) => {
                if let (Some(eng), Some(preview)) = (engine.as_ref(), query.preview.as_ref()) {
                    IpcResponse {
                        ok: true,
                        batch: Some(tricerules_proto::RuledEventBatch {
                            spell_payment_preview: Some(
                                eng.preview_spell_payment(query.player_id, preview),
                            ),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }
                } else {
                    IpcResponse {
                        error: "payment query requires a session and proposal".into(),
                        ..Default::default()
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

/// Reads one envelope, bounded by the idle timeout. `Ok(None)` means the peer sent nothing for the
/// whole window — a half-open connection whose session must be released, not a protocol error.
///
/// The timeout covers the *whole* frame, so a peer that sends a length prefix and then stalls is
/// dropped too.
async fn read_envelope(
    sock: &mut TcpStream,
    idle_timeout: Option<Duration>,
) -> Result<Option<IpcEnvelope>, Box<dyn std::error::Error + Send + Sync>> {
    match idle_timeout {
        Some(d) => match tokio::time::timeout(d, read_proto::<IpcEnvelope>(sock)).await {
            Ok(res) => res.map(Some),
            Err(_elapsed) => Ok(None),
        },
        None => read_proto::<IpcEnvelope>(sock).await.map(Some),
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

/// One wire frame: u32 BE length prefix followed by the encoded payload, in a single buffer.
/// Kept whole so the framing is testable without a socket, and so `write_proto` issues exactly one
/// `write_all` — two writes let Nagle hold the length prefix back from the payload.
fn encode_frame<M: Message>(msg: &M) -> Vec<u8> {
    let payload_len = msg.encoded_len();
    let mut frame = Vec::with_capacity(4 + payload_len);
    frame.extend_from_slice(&(payload_len as u32).to_be_bytes());
    msg.encode(&mut frame)
        .expect("Vec grows on demand, so encoding into it cannot run out of space");
    frame
}

async fn write_proto<M: Message>(
    sock: &mut TcpStream,
    msg: &M,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    sock.write_all(&encode_frame(msg)).await?;
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

    /// Both halves of the dev gate are required. The asymmetry is the point: Servatrice asking
    /// is not enough on its own, so a production sidecar (which has no env var set) cannot be
    /// talked into accepting cheat commands by anything upstream of it.
    #[test]
    fn dev_gate_needs_both_the_session_request_and_the_sidecar_env() {
        assert!(dev_commands_enabled_for_session(true, Some("1")));
        assert!(dev_commands_enabled_for_session(true, Some("true")));

        assert!(
            !dev_commands_enabled_for_session(true, None),
            "a session may ask, but an unflagged sidecar refuses"
        );
        assert!(
            !dev_commands_enabled_for_session(false, Some("1")),
            "a dev sidecar still only enables sessions that asked"
        );
        assert!(!dev_commands_enabled_for_session(false, None));
    }

    /// A connection with a game on it must get a leash no plausible game reaches — dropping it
    /// kills that game for good — while one that never became a game dies quickly.
    #[test]
    fn idle_timeout_defaults_are_short_before_a_session_and_long_during_one() {
        let t = idle_timeouts_from_env(None);
        assert_eq!(
            t.session,
            Some(Duration::from_secs(DEFAULT_SESSION_IDLE_TIMEOUT_SECS))
        );
        assert_eq!(
            t.pre_session,
            Some(Duration::from_secs(DEFAULT_PRE_SESSION_IDLE_TIMEOUT_SECS))
        );
        assert_eq!(t.for_session(true), t.session);
        assert_eq!(t.for_session(false), t.pre_session);
    }

    /// A small override must shorten *both*, or the manual dev-loop test ("set it to 20 and wait")
    /// would silently keep the 60 s pre-session default.
    #[test]
    fn a_configured_timeout_sets_the_session_leash_and_caps_the_pre_session_one() {
        let short = idle_timeouts_from_env(Some(" 20 "));
        assert_eq!(short.session, Some(Duration::from_secs(20)));
        assert_eq!(short.pre_session, Some(Duration::from_secs(20)));

        let long = idle_timeouts_from_env(Some("7200"));
        assert_eq!(long.session, Some(Duration::from_secs(7200)));
        assert_eq!(
            long.pre_session,
            Some(Duration::from_secs(DEFAULT_PRE_SESSION_IDLE_TIMEOUT_SECS)),
            "a long session leash must not stretch the pre-session one"
        );
    }

    #[test]
    fn idle_timeout_zero_disables_both_and_garbage_falls_back_to_the_defaults() {
        assert_eq!(
            idle_timeouts_from_env(Some("0")),
            IdleTimeouts {
                pre_session: None,
                session: None
            }
        );
        for value in ["", "never", "-1", "30s"] {
            assert_eq!(
                idle_timeouts_from_env(Some(value)),
                idle_timeouts_from_env(None),
                "{value:?} must not break the process, just fall back"
            );
        }
    }

    #[test]
    fn encode_frame_prefixes_the_payload_with_its_big_endian_length() {
        let resp = validate_deck_response(&names(&["Mountain"]));
        let payload = resp.encode_to_vec();
        let frame = encode_frame(&resp);

        assert_eq!(frame.len(), 4 + payload.len());
        assert_eq!(frame[..4], (payload.len() as u32).to_be_bytes());
        assert_eq!(frame[4..], payload[..]);
    }

    #[test]
    fn encode_frame_of_an_empty_message_is_a_zero_length_prefix() {
        let frame = encode_frame(&IpcEnvelope { msg: None });
        assert_eq!(frame, vec![0, 0, 0, 0]);
    }

    /// The same leash before and after a session — the shape most of these tests want.
    fn uniform_timeout(t: Option<Duration>) -> IdleTimeouts {
        IdleTimeouts {
            pre_session: t,
            session: t,
        }
    }

    /// Drives a real `handle_connection` over loopback and hands back the client end.
    async fn connect_to_handler(
        idle_timeouts: IdleTimeouts,
    ) -> (TcpStream, tokio::task::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let addr = listener.local_addr().expect("local addr");
        let server = tokio::spawn(async move {
            let (sock, _) = listener.accept().await.expect("accept");
            handle_connection(sock, idle_timeouts)
                .await
                .expect("an idle drop is Ok, not an error");
        });
        let client = TcpStream::connect(addr).await.expect("connect");
        (client, server)
    }

    /// The guard's whole point: a peer that goes silent without closing its socket must not park
    /// the task on `read_exact` forever holding a live `GameEngine`.
    #[tokio::test]
    async fn silent_peer_is_dropped_once_the_idle_timeout_elapses() {
        let (mut client, server) =
            connect_to_handler(uniform_timeout(Some(Duration::from_millis(50)))).await;

        // Send nothing at all; the server side must close on its own.
        let mut buf = [0u8; 1];
        let read = tokio::time::timeout(Duration::from_secs(5), client.read(&mut buf))
            .await
            .expect("server must close the connection, not hang");
        assert_eq!(read.expect("clean EOF"), 0, "server closed its end");

        tokio::time::timeout(Duration::from_secs(5), server)
            .await
            .expect("handler task must finish")
            .expect("handler task must not panic");
    }

    #[tokio::test]
    async fn a_disabled_idle_timeout_leaves_a_silent_peer_connected() {
        let (mut client, server) = connect_to_handler(uniform_timeout(None)).await;

        let mut buf = [0u8; 1];
        let read = tokio::time::timeout(Duration::from_millis(200), client.read(&mut buf)).await;
        assert!(read.is_err(), "no timeout configured means no idle drop");

        server.abort();
    }

    /// Round-trips a request through the rewritten single-`write_all` framing.
    #[tokio::test]
    async fn a_request_is_answered_over_the_wire_with_a_length_prefixed_frame() {
        let (mut client, server) =
            connect_to_handler(uniform_timeout(Some(Duration::from_secs(5)))).await;

        let env = IpcEnvelope {
            msg: Some(Msg::ValidateDeck(
                tricerules_proto::ruled::v1::ValidateDeck {
                    card_names: names(&["Mountain", "Black Lotus"]),
                },
            )),
        };
        client
            .write_all(&encode_frame(&env))
            .await
            .expect("write request");

        let mut lenbuf = [0u8; 4];
        client.read_exact(&mut lenbuf).await.expect("length prefix");
        let mut payload = vec![0u8; u32::from_be_bytes(lenbuf) as usize];
        client.read_exact(&mut payload).await.expect("payload");
        let resp = IpcResponse::decode(&payload[..]).expect("decode response");

        assert!(!resp.ok);
        assert_eq!(resp.missing_card_names, vec!["Black Lotus"]);

        // SessionEnd ends the loop cleanly, so the handler returns rather than idling out.
        client
            .write_all(&encode_frame(&IpcEnvelope {
                msg: Some(Msg::SessionEnd(tricerules_proto::ruled::v1::SessionEnd {})),
            }))
            .await
            .expect("write session end");
        tokio::time::timeout(Duration::from_secs(5), server)
            .await
            .expect("handler task must finish")
            .expect("handler task must not panic");
    }

    /// Reads one length-prefixed response frame from the client end.
    async fn read_response(client: &mut TcpStream) -> IpcResponse {
        let mut lenbuf = [0u8; 4];
        client.read_exact(&mut lenbuf).await.expect("length prefix");
        let mut payload = vec![0u8; u32::from_be_bytes(lenbuf) as usize];
        client.read_exact(&mut payload).await.expect("payload");
        IpcResponse::decode(&payload[..]).expect("decode response")
    }

    /// The split is the whole point: once a game exists on the connection, the short pre-session
    /// leash must no longer apply. Dropping a live session kills that game for good — Servatrice
    /// cannot restore engine state by reconnecting — so an idle-but-alive game must survive.
    #[tokio::test]
    async fn a_started_session_is_held_to_the_long_leash_not_the_pre_session_one() {
        let (mut client, server) = connect_to_handler(IdleTimeouts {
            pre_session: Some(Duration::from_millis(50)),
            session: Some(Duration::from_secs(3600)),
        })
        .await;

        client
            .write_all(&encode_frame(&IpcEnvelope {
                msg: Some(Msg::SessionStart(
                    tricerules_proto::ruled::v1::SessionStart {
                        game_id: 7,
                        seed: 42,
                        player_ids: vec![0, 1],
                        player_decks: vec![], // engine default decks
                        servatrice_build: "test".to_string(),
                        dev_commands_enabled: false,
                    },
                )),
            }))
            .await
            .expect("write session start");
        assert!(read_response(&mut client).await.ok, "session must start");

        // Well past the pre-session leash: a live game must not be dropped by it.
        let mut buf = [0u8; 1];
        let read = tokio::time::timeout(Duration::from_millis(400), client.read(&mut buf)).await;
        assert!(
            read.is_err(),
            "connection with a live session must outlive the pre-session timeout"
        );

        server.abort();
    }

    #[test]
    fn dev_env_values_other_than_1_or_true_do_not_open_the_gate() {
        for value in ["", "0", "false", "yes", "TRUE", "on"] {
            assert!(
                !dev_commands_allowed(Some(value)),
                "{value:?} must not enable dev commands"
            );
        }
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
