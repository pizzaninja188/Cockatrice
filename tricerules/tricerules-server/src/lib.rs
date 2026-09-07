//! The shared live/replay session entry point. Reconstruction must use this exact constructor,
//! deck-name resolver, dev gate, command dispatch, and initial publication cache behavior.

use std::collections::BTreeSet;
use tricerules_cards::CardRegistry;
use tricerules_core::{GameEngine, PlayerId};
use tricerules_proto::ipc_envelope::Msg;
use tricerules_proto::{IpcEnvelope, IpcResponse, PlayerDeck};
pub mod replay;

pub const ENGINE_BUILD: &str = env!("TRICERULES_BUILD_FINGERPRINT");

fn limit_diagnostic_payload(response: &mut IpcResponse) {
    use prost::Message;
    if response.diagnostic_state_json.len() > 8 * 1024 * 1024
        || response.encoded_len() > 15 * 1024 * 1024
    {
        response.diagnostic_state_json = serde_json::json!({"capture_error":"Diagnostic snapshot exceeds the capture frame budget"}).to_string();
    }
}

pub fn dev_commands_allowed(value: Option<&str>) -> bool {
    matches!(value, Some("1") | Some("true"))
}
pub fn dev_commands_enabled_for_session(requested: bool, value: Option<&str>) -> bool {
    requested && dev_commands_allowed(value)
}

pub struct EngineSession {
    pub engine: Option<GameEngine>,
    pub game_id: u64,
    dev_allowed: bool,
    effective_dev: bool,
    diagnostic_capture: bool,
}

impl EngineSession {
    pub fn new(dev_allowed: bool) -> Self {
        Self {
            engine: None,
            game_id: 0,
            dev_allowed,
            effective_dev: false,
            diagnostic_capture: false,
        }
    }

    /// None ends the connection. Inspection fields are IPC-only and never enter an event batch.
    pub fn process(&mut self, envelope: &IpcEnvelope) -> Option<IpcResponse> {
        let mut response = match envelope.msg.as_ref()? {
            Msg::SessionStart(start) => {
                self.game_id = start.game_id;
                self.diagnostic_capture = start.diagnostic_capture_enabled;
                self.effective_dev = start.dev_commands_enabled && self.dev_allowed;
                self.engine = None;
                eprintln!(
                    "tricerules: session start game {} seed {} (servatrice build {})",
                    start.game_id,
                    start.seed,
                    if start.servatrice_build.is_empty() {
                        "unknown"
                    } else {
                        &start.servatrice_build
                    }
                );
                match resolve_deck_names(&start.player_ids, &start.player_decks) {
                    Err(missing) => missing_cards_response(missing),
                    Ok(decks) => {
                        match GameEngine::new(start.seed, &start.player_ids, 20, decks, false) {
                            Err(error) => IpcResponse {
                                error: error.to_string(),
                                ..Default::default()
                            },
                            Ok(mut engine) => {
                                if self.effective_dev {
                                    eprintln!("tricerules: DEV COMMANDS ENABLED for game {} — cheat commands are accepted", start.game_id);
                                    engine.enable_dev_commands();
                                }
                                let batch = engine.initial_response_batch();
                                self.engine = Some(engine);
                                IpcResponse {
                                    ok: true,
                                    batch: Some(batch),
                                    engine_build: ENGINE_BUILD.into(),
                                    card_data_hash: CardRegistry::content_hash(),
                                    ..Default::default()
                                }
                            }
                        }
                    }
                }
            }
            Msg::ValidateDeck(deck) => validate_deck_response(&deck.card_names),
            Msg::PlayerCommand(command) => match self.engine.as_mut() {
                Some(engine) => {
                    engine.player_command_ipc(command.player_id, &command.ruled_command)
                }
                None => IpcResponse {
                    error: "no session".into(),
                    ..Default::default()
                },
            },
            Msg::PaymentQuery(query) => match (self.engine.as_ref(), query.preview.as_ref()) {
                (Some(engine), Some(preview)) => IpcResponse {
                    ok: true,
                    batch: Some(tricerules_proto::RuledEventBatch {
                        payment_preview: Some(engine.preview_payment(query.player_id, preview)),
                        ..Default::default()
                    }),
                    ..Default::default()
                },
                _ => IpcResponse {
                    error: "payment query requires a session and proposal".into(),
                    ..Default::default()
                },
            },
            Msg::SessionEnd(_) => return None,
        };
        if let Some(engine) = &self.engine {
            response.effective_dev_commands_enabled = self.effective_dev;
            response.diagnostic_command_index = engine.state.command_index;
            if self.diagnostic_capture {
                response.diagnostic_state_json = match engine.diagnostic_snapshot() {
                    Ok(state) => state.to_string(),
                    Err(error) => {
                        serde_json::json!({"capture_error": error.to_string()}).to_string()
                    }
                };
                limit_diagnostic_payload(&mut response);
            }
        }
        Some(response)
    }
}

pub fn missing_cards_response(missing: Vec<String>) -> IpcResponse {
    IpcResponse {
        error: format!("unimplemented cards: {}", missing.join(", ")),
        missing_card_names: missing,
        ..Default::default()
    }
}

pub fn validate_deck_response(card_names: &[String]) -> IpcResponse {
    let registry = CardRegistry::global();
    let missing: BTreeSet<_> = card_names
        .iter()
        .filter(|name| registry.id_for_name(name).is_none())
        .map(|name| name.trim().to_owned())
        .collect();
    if missing.is_empty() {
        IpcResponse {
            ok: true,
            ..Default::default()
        }
    } else {
        missing_cards_response(missing.into_iter().collect())
    }
}

/// Keep deck ordering and duplicates; resolve identity only through the engine registry.
pub fn resolve_deck_names(
    pids: &[PlayerId],
    decks: &[PlayerDeck],
) -> Result<Option<Vec<Vec<String>>>, Vec<String>> {
    if decks.is_empty() {
        return Ok(None);
    }
    let registry = CardRegistry::global();
    let mut out: Vec<Vec<String>> = pids.iter().map(|_| vec![]).collect();
    let mut missing = BTreeSet::new();
    for deck in decks {
        let Some(index) = pids.iter().position(|&id| id == deck.player_id) else {
            continue;
        };
        for name in &deck.mainboard_card_name {
            match registry.id_for_name(name) {
                Some(id) => out[index].push(id.to_owned()),
                None => {
                    missing.insert(name.trim().to_owned());
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

#[cfg(test)]
mod diagnostic_tests {
    use super::*;
    #[test]
    fn oversized_diagnostics_do_not_break_the_gameplay_response() {
        let mut response = IpcResponse {
            ok: true,
            diagnostic_state_json: "x".repeat(9 * 1024 * 1024),
            batch: Some(Default::default()),
            ..Default::default()
        };
        limit_diagnostic_payload(&mut response);
        assert!(response.ok);
        assert!(response.batch.is_some());
        assert!(response.diagnostic_state_json.len() < 1024);
        assert!(response.diagnostic_state_json.contains("capture_error"));
    }
}
