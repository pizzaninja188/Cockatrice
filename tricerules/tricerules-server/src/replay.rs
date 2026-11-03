//! Bounded capture reader and deterministic reconstruction through EngineSession.

use crate::{EngineSession, ENGINE_BUILD};
use prost::Message;
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};
use tricerules_proto::{diagnostics::Record, ipc_envelope::Msg, IpcEnvelope, IpcResponse};

#[derive(Debug)]
pub struct Exchange {
    pub sequence: u64,
    pub request: IpcEnvelope,
    pub response: Option<IpcResponse>,
}
#[derive(Debug)]
pub struct Capture {
    pub directory: PathBuf,
    pub manifest: Value,
    pub exchanges: Vec<Exchange>,
    pub warnings: Vec<String>,
}
#[derive(Default)]
pub struct ReplayOptions {
    pub stop_after: Option<u64>,
    pub retry_sequence: Option<u64>,
    pub allow_build_mismatch: bool,
}
#[derive(Debug)]
pub struct ReplayResult {
    pub accepted_commands: u64,
    pub state: Value,
    pub differences: Vec<Value>,
    pub warnings: Vec<String>,
    pub retry: Option<Value>,
}
pub fn build_resume_plan(
    capture: &Capture,
    options: &ReplayOptions,
) -> Result<tricerules_proto::diagnostics::ResumePlan, String> {
    use sha2::{Digest, Sha256};
    use tricerules_proto::{
        commander_setup,
        diagnostics::{ResumePlan, ResumeStep},
        ruled_command::Cmd,
        AutoPassPolicy, CommanderSetup, PhaseId, PlayerDeck, RuledCommand,
    };
    if options.retry_sequence.is_some() {
        return Err("Live resume requires an accepted prefix, not an isolated retry".into());
    }
    let verified = reconstruct(capture, options)?;
    if !verified.differences.is_empty() && !options.allow_build_mismatch {
        return Err("Capture diverges; refusing live resume".into());
    }
    let effective_dev = capture.manifest["effective_dev_commands_enabled"]
        .as_bool()
        .unwrap_or(false);
    let mut session = EngineSession::new(effective_dev);
    let first = &capture.exchanges[0];
    let Some(Msg::SessionStart(start)) = &first.request.msg else {
        return Err("missing startup".into());
    };
    let response = session
        .process(&first.request)
        .ok_or("startup ended session")?;
    let mut plan = ResumePlan {
        format_version: 1,
        parent_capture_id: capture.manifest["capture_id"]
            .as_str()
            .ok_or("missing capture id")?
            .into(),
        engine_build: ENGINE_BUILD.into(),
        card_data_hash: response.card_data_hash.clone(),
        session_start: Some(start.clone()),
        stop_after: verified.accepted_commands,
        effective_dev_commands_enabled: effective_dev,
        initial_state_sha256: Sha256::digest(response.diagnostic_state_json.as_bytes()).to_vec(),
        comparison_mode: options.allow_build_mismatch,
        ..Default::default()
    };
    let engine = session
        .engine
        .as_ref()
        .ok_or("startup produced no engine")?;
    let registry = tricerules_cards::registry::global();
    plan.display_decks = if start.player_decks.is_empty() {
        engine
            .state
            .players
            .iter()
            .map(|player| {
                let names = player
                    .library
                    .iter()
                    .chain(player.hand.iter())
                    .map(|oid| {
                        let object = engine
                            .state
                            .objects
                            .get(oid)
                            .ok_or("startup object missing")?;
                        Ok(registry
                            .get(&object.card_id)
                            .ok_or("startup card definition missing")?
                            .name
                            .clone())
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                let commanders = player
                    .command_zone
                    .iter()
                    .map(|oid| {
                        let object = engine
                            .state
                            .objects
                            .get(oid)
                            .ok_or("Commander object missing")?;
                        Ok(registry
                            .get(&object.card_id)
                            .ok_or("Commander card definition missing")?
                            .name
                            .clone())
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                let setup = if commanders.is_empty() {
                    commander_setup::Declaration::NoCommander(true)
                } else {
                    commander_setup::Declaration::Declared(commander_setup::Declared {
                        card_name: commanders,
                    })
                };
                Ok(PlayerDeck {
                    player_id: player.id,
                    mainboard_card_name: names,
                    commander_setup: Some(CommanderSetup {
                        declaration: Some(setup),
                    }),
                })
            })
            .collect::<Result<Vec<_>, String>>()?
    } else {
        start.player_decks.clone()
    };
    let stops: Vec<_> = [
        PhaseId::Upkeep,
        PhaseId::Draw,
        PhaseId::Main1,
        PhaseId::BeginCombat,
        PhaseId::DeclareAttackers,
        PhaseId::DeclareBlockers,
        PhaseId::FirstStrikeDamage,
        PhaseId::CombatDamage,
        PhaseId::EndCombat,
        PhaseId::Main2,
        PhaseId::EndStep,
    ]
    .iter()
    .map(|phase| *phase as i32)
    .collect();
    plan.restored_auto_pass_policies = start
        .player_ids
        .iter()
        .map(|player_id| AutoPassPolicy {
            player_id: *player_id,
            stop_on_own_turn: stops.clone(),
            stop_on_opponent_turn: stops.clone(),
        })
        .collect();
    for exchange in capture.exchanges.iter().skip(1) {
        if plan.steps.len() as u64 >= plan.stop_after {
            break;
        }
        let Some(expected) = &exchange.response else {
            break;
        };
        if !expected.ok {
            continue;
        }
        let response = session
            .process(&exchange.request)
            .ok_or("unexpected session end")?;
        if !response.ok {
            return Err(format!("Command {} no longer accepted", exchange.sequence));
        }
        if let Some(Msg::PlayerCommand(command)) = &exchange.request.msg {
            let parsed = RuledCommand::decode(command.ruled_command.as_slice())
                .map_err(|e| e.to_string())?;
            if let Some(Cmd::CanonicalGameplay(canonical)) = parsed.cmd {
                plan.restored_auto_pass_policies = canonical.auto_pass_policies;
            }
            plan.steps.push(ResumeStep {
                source_sequence: exchange.sequence,
                command: Some(command.clone()),
                expected_state_sha256: Sha256::digest(response.diagnostic_state_json.as_bytes())
                    .to_vec(),
            });
        }
    }
    Ok(plan)
}
pub fn load_capture(directory: &Path) -> Result<Capture, String> {
    let directory = directory
        .canonicalize()
        .map_err(|e| format!("capture directory: {e}"))?;
    let manifest: Value = serde_json::from_slice(&bounded_file(
        &directory.join("manifest.json"),
        1024 * 1024,
    )?)
    .map_err(|e| format!("manifest: {e}"))?;
    if manifest["format_version"] != 1
        || manifest["privacy"] != "server_only"
        || manifest["source"] != "server"
    {
        return Err("Reconstruction requires a version-1 maintainer server capture, not a client export or legacy replay".into());
    }
    let file =
        fs::File::open(directory.join("timeline.jsonl")).map_err(|e| format!("timeline: {e}"))?;
    let mut reader = BufReader::new(file);
    let mut exchanges: Vec<Exchange> = vec![];
    let mut requests = BTreeMap::new();
    let mut warnings = vec![];
    let mut previous_sequence = 0;
    loop {
        let mut line = vec![];
        (&mut reader)
            .take(32 * 1024 * 1024 + 1)
            .read_until(b'\n', &mut line)
            .map_err(|e| e.to_string())?;
        if line.is_empty() {
            break;
        }
        if line.len() > 32 * 1024 * 1024 {
            return Err("timeline record exceeds 32 MiB".into());
        }
        if !line.ends_with(b"\n") {
            warnings.push(
                "Truncated/uncommitted final timeline record; only the valid prefix is available"
                    .into(),
            );
            break;
        }
        let row: Value = serde_json::from_slice(&line)
            .map_err(|e| format!("timeline after sequence {previous_sequence}: {e}"))?;
        let sequence = row["sequence"]
            .as_str()
            .and_then(|v| v.parse::<u64>().ok())
            .ok_or("invalid sequence")?;
        if row["format_version"] != 1 || sequence != previous_sequence + 1 {
            return Err(format!("unsupported version or sequence gap at {sequence}"));
        }
        previous_sequence = sequence;
        let kind = row["kind"].as_str().ok_or("missing record kind")?;
        if kind == "capture_gap" {
            warnings.push(format!(
                "Capture gap at sequence {sequence}: {}",
                row["data"]
            ));
            break;
        }
        if kind != "engine_request" && kind != "engine_response" {
            continue;
        }
        let relative = row["raw_ref"].as_str().ok_or("missing raw path")?;
        if relative != format!("raw/{sequence:012}.pb") {
            return Err(format!("invalid raw path at {sequence}"));
        }
        let raw_path = directory
            .join(relative)
            .canonicalize()
            .map_err(|e| format!("raw path: {e}"))?;
        if !raw_path.starts_with(&directory) {
            return Err("raw path escapes capture".into());
        }
        let raw = Record::decode(bounded_file(&raw_path, 32 * 1024 * 1024)?.as_slice())
            .map_err(|e| format!("raw record {sequence}: {e}"))?;
        if raw.format_version != 1
            || raw.sequence != sequence
            || raw.kind != kind
            || row["message_type"] != raw.message_type
            || row["correlation_id"] != raw.correlation_id
        {
            return Err(format!("raw/timeline metadata mismatch at {sequence}"));
        }
        if kind == "engine_request" {
            if raw.message_type != "ruled.v1.IpcEnvelope"
                || requests.contains_key(&raw.correlation_id)
            {
                return Err(format!("duplicate or invalid engine request at {sequence}"));
            }
            let request = IpcEnvelope::decode(raw.payload.as_slice())
                .map_err(|e| format!("request {sequence}: {e}"))?;
            requests.insert(raw.correlation_id, exchanges.len());
            exchanges.push(Exchange {
                sequence,
                request,
                response: None,
            });
        } else {
            if raw.message_type != "ruled.v1.IpcResponse" {
                return Err("invalid response type".into());
            }
            let index = *requests
                .get(&raw.correlation_id)
                .ok_or("response without request")?;
            if exchanges[index].response.is_some() {
                return Err("duplicate response".into());
            }
            exchanges[index].response = Some(
                IpcResponse::decode(raw.payload.as_slice())
                    .map_err(|e| format!("response {sequence}: {e}"))?,
            );
        }
    }
    Ok(Capture {
        directory,
        manifest,
        exchanges,
        warnings,
    })
}

fn bounded_file(path: &Path, limit: u64) -> Result<Vec<u8>, String> {
    let file = fs::File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if file.metadata().map_err(|e| e.to_string())?.len() > limit {
        return Err(format!("{} exceeds size limit", path.display()));
    }
    let mut bytes = vec![];
    file.take(limit + 1)
        .read_to_end(&mut bytes)
        .map_err(|e| e.to_string())?;
    if bytes.len() as u64 > limit {
        return Err("file grew beyond size limit".into());
    }
    Ok(bytes)
}

pub fn normalized_response(response: &IpcResponse) -> Result<Value, String> {
    let mut value =
        tricerules_core::diagnostic_json::to_value(response).map_err(|e| e.to_string())?;
    value["diagnostic_state_json"] = if response.diagnostic_state_json.is_empty() {
        Value::Null
    } else {
        serde_json::from_str(&response.diagnostic_state_json)
            .map_err(|e| format!("engine snapshot: {e}"))?
    };
    Ok(value)
}

pub fn differences(before: &Value, after: &Value) -> Vec<Value> {
    fn visit(before: Option<&Value>, after: Option<&Value>, path: &str, out: &mut Vec<Value>) {
        if before == after {
            return;
        }
        if let (Some(Value::Object(a)), Some(Value::Object(b))) = (before, after) {
            let mut keys: Vec<_> = a.keys().chain(b.keys()).collect();
            keys.sort();
            keys.dedup();
            for key in keys {
                visit(
                    a.get(key),
                    b.get(key),
                    &format!("{path}/{}", key.replace('~', "~0").replace('/', "~1")),
                    out,
                );
            }
            return;
        }
        if let (Some(Value::Array(a)), Some(Value::Array(b))) = (before, after) {
            if a.len() == b.len() {
                for (index, (a, b)) in a.iter().zip(b).enumerate() {
                    visit(Some(a), Some(b), &format!("{path}/{index}"), out);
                }
                return;
            }
        }
        let mut row = serde_json::json!({"path":path,"operation":if before.is_none() { "add" } else if after.is_none() { "remove" } else { "replace" }});
        if let Some(before) = before {
            row["before"] = before.clone();
        }
        if let Some(after) = after {
            row["after"] = after.clone();
        }
        out.push(row);
    }
    let mut result = vec![];
    visit(Some(before), Some(after), "", &mut result);
    result
}

pub fn reconstruct(capture: &Capture, options: &ReplayOptions) -> Result<ReplayResult, String> {
    if options.stop_after.is_some() && options.retry_sequence.is_some() {
        return Err("Choose stop-after or retry-sequence, not both".into());
    }
    let build_matches = capture.manifest["engine_build"].as_str() == Some(ENGINE_BUILD);
    let data_matches = capture.manifest["card_data_hash"].as_str()
        == Some(tricerules_cards::registry::content_hash().as_str());
    if (!build_matches || !data_matches) && !options.allow_build_mismatch {
        return Err("Engine build or card-data hash differs; use the original build or explicit --allow-build-mismatch comparison mode".into());
    }
    let Some(first) = capture.exchanges.first() else {
        return Err("capture has no engine startup".into());
    };
    if !matches!(first.request.msg, Some(Msg::SessionStart(_)))
        || !first.response.as_ref().is_some_and(|r| r.ok)
    {
        return Err("capture has no successfully completed startup".into());
    }
    let mut session = EngineSession::new(
        capture.manifest["effective_dev_commands_enabled"]
            .as_bool()
            .unwrap_or(false),
    );
    let mut result = ReplayResult {
        accepted_commands: 0,
        state: Value::Null,
        differences: vec![],
        warnings: capture.warnings.clone(),
        retry: None,
    };
    if !build_matches || !data_matches {
        result
            .warnings
            .push("Comparison mode: build/card data differs from the capture".into());
    }
    for (index, exchange) in capture.exchanges.iter().enumerate() {
        if index > 0 && matches!(exchange.request.msg, Some(Msg::SessionStart(_))) {
            return Err("multiple startups in one capture".into());
        }
        if index > 0 && options.stop_after == Some(result.accepted_commands) {
            break;
        }
        if options.retry_sequence == Some(exchange.sequence) {
            if index == 0 {
                return Err("retry-sequence must select a gameplay request or query".into());
            }
            let response = session
                .process(&exchange.request)
                .ok_or("request ends the session")?;
            result.retry = Some(normalized_response(&response)?);
            break;
        }
        let Some(expected) = &exchange.response else {
            result.warnings.push(format!(
                "Request {} has no recorded outcome; stopped at the last proven accepted state",
                exchange.sequence
            ));
            break;
        };
        // Rejected attempts are evidence, not members of the deterministic gameplay log.
        if !expected.ok {
            continue;
        }
        let actual = session
            .process(&exchange.request)
            .ok_or("unexpected session end in replay")?;
        let changes = differences(
            &normalized_response(expected)?,
            &normalized_response(&actual)?,
        );
        if !changes.is_empty() {
            result.differences.push(
                serde_json::json!({"request_sequence":exchange.sequence.to_string(),
                "accepted_commands_before":result.accepted_commands.to_string(),"changes":changes}),
            );
        }
        if !actual.ok {
            result.warnings.push(format!(
                "Previously accepted request {} now fails: {}",
                exchange.sequence, actual.error
            ));
            break;
        }
        if matches!(exchange.request.msg, Some(Msg::PlayerCommand(_))) {
            result.accepted_commands += 1;
        }
    }
    if let Some(count) = options.stop_after {
        if result.accepted_commands != count {
            return Err(format!(
                "requested {count} accepted commands, but only {} could be reconstructed",
                result.accepted_commands
            ));
        }
    }
    if options.retry_sequence.is_some() && result.retry.is_none() {
        return Err("retry request is not in the available prefix".into());
    }
    result.state = session
        .engine
        .as_ref()
        .ok_or("replay has no engine")?
        .diagnostic_snapshot()
        .map_err(|e| e.to_string())?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    mod gemstone_normal {
        include!("replay_gemstone_normal.rs");
    }
    mod kaya_inexorable_normal {
        include!("replay_kaya_inexorable_normal.rs");
    }
    mod tamiyo_normal {
        include!("replay_tamiyo_normal.rs");
    }
    mod professor_onyx_normal {
        include!("replay_professor_onyx_normal.rs");
    }
    mod vraska_normal {
        include!("replay_vraska_normal.rs");
    }
    use super::*;
    use crate::{EngineSession, ENGINE_BUILD};
    use prost::Message;
    use std::fs;
    use tricerules_proto::{
        commander_setup,
        diagnostics::Record,
        ipc_envelope::Msg,
        ruled::v1::{
            dev_command, ruled_event::Ev, AbilitySourceZone, ActivateAbility, BattlefieldObject,
            BeginAbilityActivation, BeginSpellCast, CastMethod, CastSource, ChoiceKind,
            CommitAbilityActivation, CommitSpellCast, CostObjectRef, CostSelection,
            CounterRemovalContribution, CounterRemovalGroupSelection, DevAddMana, DevCommand,
            DevMoveCard, DevPutCardInZone, DevZone, DiscardToHandSize, FlexPipPayment,
            HandActionKind, LandSource, PassPriority, PaymentMana, PaymentSelection, PlayLand,
            PresentationPathKind, ResolutionChoiceDecision, SelectedSpellMode,
            SubmitResolutionChoice, TargetRef, TargetRefKind,
        },
        ruled_command::Cmd,
        ChooseStartingPlayer, CommanderSetup, MulliganDecision, PaymentQuery, PlayerCommand,
        PlayerDeck, PreviewPayment, RuledCommand, SessionStart, ValidateDeck,
    };

    const TEKUTHAL_NORMAL_PILOT_ID: i32 = 17;
    const TEKUTHAL_NORMAL_OPPONENT_ID: i32 = 29;

    fn tekuthal_normal_pilot_decks() -> Vec<PlayerDeck> {
        let mut pilot = Vec::new();
        for card_name in [
            "Tekuthal, Inquiry Dominus",
            "Everflowing Chalice",
            "Replicating Ring",
            "Karn's Bastion",
        ] {
            pilot.extend(vec![card_name.to_string(); 4]);
        }
        pilot.extend(vec!["Island".to_string(); 44]);

        let mut opponent = vec!["Murder".to_string(); 4];
        opponent.extend(vec!["Swamp".to_string(); 56]);

        vec![
            PlayerDeck {
                player_id: TEKUTHAL_NORMAL_PILOT_ID,
                mainboard_card_name: pilot,
                commander_setup: Some(CommanderSetup {
                    declaration: Some(commander_setup::Declaration::NoCommander(true)),
                }),
            },
            PlayerDeck {
                player_id: TEKUTHAL_NORMAL_OPPONENT_ID,
                mainboard_card_name: opponent,
                commander_setup: Some(CommanderSetup {
                    declaration: Some(commander_setup::Declaration::NoCommander(true)),
                }),
            },
        ]
    }

    fn tekuthal_normal_assert_decks(player_decks: &[PlayerDeck]) {
        let registry = tricerules_cards::registry::global();
        assert_eq!(player_decks.len(), 2);
        for (deck, expected_player_id) in player_decks
            .iter()
            .zip([TEKUTHAL_NORMAL_PILOT_ID, TEKUTHAL_NORMAL_OPPONENT_ID])
        {
            assert_eq!(deck.player_id, expected_player_id);
            assert_eq!(deck.mainboard_card_name.len(), 60);
            assert!(matches!(
                deck.commander_setup
                    .as_ref()
                    .and_then(|setup| setup.declaration.as_ref()),
                Some(commander_setup::Declaration::NoCommander(true))
            ));
            for card_name in deck
                .mainboard_card_name
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
            {
                assert!(
                    registry.id_for_name(card_name).is_some(),
                    "deck card name is absent from the global registry: {card_name}"
                );
                let copies = deck
                    .mainboard_card_name
                    .iter()
                    .filter(|candidate| *candidate == card_name)
                    .count();
                if matches!(card_name.as_str(), "Island" | "Swamp") {
                    assert!(
                        copies > 4,
                        "basic lands are not subject to the nonbasic cap"
                    );
                } else {
                    assert!(copies <= 4, "too many copies of nonbasic {card_name}");
                }
            }
        }
    }

    fn tekuthal_normal_player_command(
        session: &mut EngineSession,
        directory: &Path,
        rows: &mut Vec<String>,
        accepted_commands: &mut usize,
        player_id: i32,
        command: RuledCommand,
        battlefield: &mut Vec<BattlefieldObject>,
    ) -> IpcResponse {
        let response = record_player_command(session, directory, rows, player_id, command);
        assert!(
            response.ok,
            "player {player_id} command rejected: {}",
            response.error
        );
        assert!(
            response.batch.is_some(),
            "accepted command returns its current batch"
        );
        *accepted_commands += 1;
        kaya_capture_battlefield(&response, TEKUTHAL_NORMAL_PILOT_ID, battlefield);
        response
    }

    fn tekuthal_normal_card_records(
        state: &tricerules_core::state::GameState,
        object_ids: impl IntoIterator<Item = tricerules_core::state::ObjectId>,
    ) -> Vec<(tricerules_core::state::ObjectId, String, String)> {
        let registry = tricerules_cards::registry::global();
        object_ids
            .into_iter()
            .map(|object_id| {
                let object = state
                    .objects
                    .get(&object_id)
                    .expect("starting hand or library object remains in the game state");
                let definition = registry
                    .get(&object.card_id)
                    .expect("physical card id resolves through the global registry");
                (object_id, object.card_id.clone(), definition.name.clone())
            })
            .collect()
    }

    fn tekuthal_normal_advance_to_main(
        session: &mut EngineSession,
        directory: &Path,
        rows: &mut Vec<String>,
        accepted_commands: &mut usize,
        mut latest: IpcResponse,
        battlefield: &mut Vec<BattlefieldObject>,
        player_id: i32,
    ) -> IpcResponse {
        for _ in 0..128 {
            let state = &session
                .engine
                .as_ref()
                .expect("normal Tekuthal session has an engine")
                .state;
            if state.turn_step == tricerules_core::TurnStep::Main1
                && state.players[state.active_player_idx].id == player_id
                && state.priority_player_id() == player_id
            {
                return latest;
            }
            let priority_player = state.priority_player_id();
            latest = tekuthal_normal_player_command(
                session,
                directory,
                rows,
                accepted_commands,
                priority_player,
                RuledCommand {
                    cmd: Some(Cmd::PassPriority(PassPriority {})),
                },
                battlefield,
            );
        }
        panic!("normal priority passes did not reach player {player_id}'s Main1");
    }

    struct TekuthalNormalLandPlan<'a> {
        player_id: i32,
        card_name: &'a str,
    }

    struct TekuthalNormalCastPlan<'a> {
        card_name: &'a str,
        expected_total_cost: &'a str,
        blue_mana: u32,
        kick_once: bool,
    }

    fn tekuthal_normal_play_land(
        session: &mut EngineSession,
        directory: &Path,
        rows: &mut Vec<String>,
        accepted_commands: &mut usize,
        latest: &IpcResponse,
        battlefield: &mut Vec<BattlefieldObject>,
        plan: TekuthalNormalLandPlan<'_>,
    ) -> IpcResponse {
        let TekuthalNormalLandPlan {
            player_id,
            card_name,
        } = plan;
        let offer = kaya_batch(latest)
            .legal_by_player
            .get(&player_id)
            .expect("active player receives current land offers")
            .hand_actions
            .iter()
            .find(|offer| {
                offer.kind == HandActionKind::HandActionPlayLand as i32
                    && offer.card_name == card_name
            })
            .cloned()
            .unwrap_or_else(|| panic!("current hand offer includes {card_name}"));
        tekuthal_normal_player_command(
            session,
            directory,
            rows,
            accepted_commands,
            player_id,
            RuledCommand {
                cmd: Some(Cmd::PlayLand(PlayLand {
                    source: Some(LandSource {
                        location: Some(
                            tricerules_proto::ruled::v1::land_source::Location::HandIndex(
                                offer.hand_index,
                            ),
                        ),
                        ..Default::default()
                    }),
                    ..Default::default()
                })),
            },
            battlefield,
        )
    }

    fn tekuthal_normal_resolve_stack(
        session: &mut EngineSession,
        directory: &Path,
        rows: &mut Vec<String>,
        accepted_commands: &mut usize,
        mut latest: IpcResponse,
        battlefield: &mut Vec<BattlefieldObject>,
    ) -> IpcResponse {
        for _ in 0..8 {
            let state = &session
                .engine
                .as_ref()
                .expect("normal Tekuthal session has an engine")
                .state;
            if state.stack.is_empty() {
                return latest;
            }
            let priority_player = state.priority_player_id();
            latest = tekuthal_normal_player_command(
                session,
                directory,
                rows,
                accepted_commands,
                priority_player,
                RuledCommand {
                    cmd: Some(Cmd::PassPriority(PassPriority {})),
                },
                battlefield,
            );
        }
        assert!(
            session
                .engine
                .as_ref()
                .expect("normal Tekuthal session has an engine")
                .state
                .stack
                .is_empty(),
            "ordinary recorded priority passes resolve the current spell"
        );
        latest
    }

    fn tekuthal_normal_cast_from_current_offer(
        session: &mut EngineSession,
        directory: &Path,
        rows: &mut Vec<String>,
        accepted_commands: &mut usize,
        mut latest: IpcResponse,
        battlefield: &mut Vec<BattlefieldObject>,
        plan: TekuthalNormalCastPlan<'_>,
    ) -> IpcResponse {
        let TekuthalNormalCastPlan {
            card_name,
            expected_total_cost,
            blue_mana,
            kick_once,
        } = plan;
        let spell = kaya_batch(&latest)
            .legal_by_player
            .get(&TEKUTHAL_NORMAL_PILOT_ID)
            .expect("P17 receives current legal hand actions")
            .hand_actions
            .iter()
            .find(|offer| {
                offer.kind == HandActionKind::HandActionCastSpell as i32
                    && offer.card_name == card_name
            })
            .cloned()
            .unwrap_or_else(|| panic!("current hand offer contains castable {card_name}"));
        let cast_cost_group_selections = if kick_once {
            let (group_index, option_index, maximum_repetitions) = spell
                .cost_choices
                .as_ref()
                .expect("current Chalice offer publishes kicker choices")
                .cast_cost_groups
                .iter()
                .find_map(|group| {
                    group
                        .options
                        .iter()
                        .find(|option| option.selectable && option.maximum_repetitions.is_some())
                        .map(|option| {
                            (
                                group.group_index,
                                option.option_index,
                                option.maximum_repetitions.unwrap(),
                            )
                        })
                })
                .expect("current Chalice offer has a selectable repeated-cost option");
            assert!(maximum_repetitions >= 1);
            println!(
                "Chalice current kicker offer group={group_index} option={option_index} maximum_repetitions={maximum_repetitions}; selecting one"
            );
            vec![tricerules_proto::ruled::v1::CastCostGroupSelection {
                group_index,
                option_index,
                repetitions: Some(1),
                ..Default::default()
            }]
        } else {
            Vec::new()
        };

        latest = tekuthal_normal_player_command(
            session,
            directory,
            rows,
            accepted_commands,
            TEKUTHAL_NORMAL_PILOT_ID,
            RuledCommand {
                cmd: Some(Cmd::BeginSpellCast(BeginSpellCast {
                    announcement: Some(tricerules_proto::ruled::v1::SpellCastAnnouncement {
                        source: Some(CastSource {
                            location: Some(
                                tricerules_proto::ruled::v1::cast_source::Location::HandIndex(
                                    spell.hand_index,
                                ),
                            ),
                            expected_zone_change_generation: None,
                        }),
                        face_index: spell.face_index,
                        cast_cost_group_selections,
                        cast_method: CastMethod::Normal as i32,
                        ..Default::default()
                    }),
                })),
            },
            battlefield,
        );
        let pending = kaya_batch(&latest)
            .legal_by_player
            .get(&TEKUTHAL_NORMAL_PILOT_ID)
            .and_then(|actions| actions.pending_spell_cast.as_ref())
            .expect("current normal spell cast publishes a pending payment");
        let payment = pending
            .payment_preview
            .as_ref()
            .expect("pending spell publishes its current payment preview");
        assert_eq!(
            payment.total_cost, expected_total_cost,
            "{card_name} current total cost"
        );

        let mut produced = PaymentMana::default();
        let island_card_id = tricerules_cards::registry::global()
            .id_for_name("Island")
            .expect("Island is in the global registry");
        for _ in 0..blue_mana {
            let (object_id, generation, ability_index, output) = battlefield
                .iter()
                .filter(|permanent| permanent.card_id == island_card_id && !permanent.tapped)
                .flat_map(|permanent| {
                    permanent
                        .activated_abilities
                        .iter()
                        .filter(|ability| ability.is_mana_ability && ability.activatable)
                        .map(|ability| {
                            (
                                permanent.object_id,
                                permanent.zone_change_generation,
                                ability.ability_index,
                                ability.mana_produced.clone(),
                            )
                        })
                })
                .find(|source| kaya_output_has_color(&source.3, 'U'))
                .unwrap_or_else(|| panic!("current untapped Islands can pay {card_name}"));
            assert!(kaya_output_has_color(&output, 'U'));
            latest = tekuthal_normal_player_command(
                session,
                directory,
                rows,
                accepted_commands,
                TEKUTHAL_NORMAL_PILOT_ID,
                RuledCommand {
                    cmd: Some(Cmd::ActivateAbility(ActivateAbility {
                        source_object_id: object_id,
                        ability_index,
                        source_zone: AbilitySourceZone::Battlefield as i32,
                        expected_zone_change_generation: generation,
                        ..Default::default()
                    })),
                },
                battlefield,
            );
            produced.u += 1;
            let pool = &session
                .engine
                .as_ref()
                .expect("normal session has an engine after Island mana")
                .state
                .players
                .iter()
                .find(|player| player.id == TEKUTHAL_NORMAL_PILOT_ID)
                .expect("P17 remains in the normal session")
                .mana_pool;
            assert_eq!(pool.blue, produced.u);
            assert_eq!(pool.colorless, 0);
            println!(
                "{card_name} current Island mana source ObjectId={object_id} generation={generation} ability={ability_index} output={output}; blue pool={}",
                pool.blue
            );
        }

        let pending = kaya_batch(&latest)
            .legal_by_player
            .get(&TEKUTHAL_NORMAL_PILOT_ID)
            .and_then(|actions| actions.pending_spell_cast.as_ref())
            .expect("pending spell cast survives ordinary Island activations");
        let payment = pending
            .payment_preview
            .as_ref()
            .expect("current pending spell publishes the updated payment preview");
        let transaction_id = pending.transaction_id;
        let revision = payment.revision;
        let payment_request = IpcEnvelope {
            msg: Some(Msg::PaymentQuery(PaymentQuery {
                player_id: TEKUTHAL_NORMAL_PILOT_ID,
                preview: Some(PreviewPayment {
                    transaction_id,
                    revision,
                    commit_spell_cast: Some(CommitSpellCast {
                        transaction_id,
                        payment: Some(PaymentSelection {
                            expected_state_revision: revision,
                            mana: Some(produced),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
            })),
        };
        let payment_response = session
            .process(&payment_request)
            .expect("PaymentQuery keeps the normal Tekuthal session open");
        pair(directory, rows, &payment_request, &payment_response);
        assert!(
            payment_response.ok,
            "{card_name} PaymentQuery: {}",
            payment_response.error
        );
        let preview = payment_response
            .batch
            .as_ref()
            .and_then(|batch| batch.payment_preview.as_ref())
            .expect("PaymentQuery returns the current payment preview");
        assert!(
            preview.valid && preview.complete,
            "{card_name} preview: {preview:?}"
        );
        assert_eq!(preview.total_cost, expected_total_cost);
        let selection = preview
            .selection
            .clone()
            .expect("complete PaymentQuery returns an exact selection");
        assert_eq!(selection.mana.as_ref(), Some(&produced));
        let restricted_mana = preview.restricted_mana.clone();
        let transaction_id = preview.transaction_id;
        println!(
            "{card_name} PaymentQuery accepted transaction={transaction_id} revision={revision} selection={selection:?} restricted_mana={restricted_mana:?}"
        );
        latest = tekuthal_normal_player_command(
            session,
            directory,
            rows,
            accepted_commands,
            TEKUTHAL_NORMAL_PILOT_ID,
            RuledCommand {
                cmd: Some(Cmd::CommitSpellCast(CommitSpellCast {
                    transaction_id,
                    payment: Some(selection),
                    restricted_mana,
                })),
            },
            battlefield,
        );
        assert!(
            session
                .engine
                .as_ref()
                .expect("normal session has an engine after spell payment")
                .state
                .pending_spell_cast
                .is_none(),
            "{card_name} commit clears the pending spell cast"
        );
        tekuthal_normal_resolve_stack(
            session,
            directory,
            rows,
            accepted_commands,
            latest,
            battlefield,
        )
    }

    fn tekuthal_normal_activate_bastion(
        session: &mut EngineSession,
        directory: &Path,
        rows: &mut Vec<String>,
        accepted_commands: &mut usize,
        mut latest: IpcResponse,
        battlefield: &mut Vec<BattlefieldObject>,
    ) -> IpcResponse {
        let registry = tricerules_cards::registry::global();
        let bastion_card_id = registry
            .id_for_name("Karn's Bastion")
            .expect("Karn's Bastion is in the global registry")
            .to_owned();
        let current_actions = kaya_batch(&latest)
            .legal_by_player
            .get(&TEKUTHAL_NORMAL_PILOT_ID)
            .expect("current T5 response includes P17's legal actions");
        assert!(current_actions.pending_spell_cast.is_none());
        assert!(current_actions.pending_ability_activation.is_none());
        kaya_capture_battlefield(&latest, TEKUTHAL_NORMAL_PILOT_ID, battlefield);

        let island_card_id = registry
            .id_for_name("Island")
            .expect("Island is in the global registry")
            .to_owned();
        let mut produced = PaymentMana::default();
        for _ in 0..4 {
            let (object_id, generation, ability_index, output) = battlefield
                .iter()
                .filter(|permanent| permanent.card_id == island_card_id && !permanent.tapped)
                .flat_map(|permanent| {
                    permanent
                        .activated_abilities
                        .iter()
                        .filter(|mana| mana.is_mana_ability && mana.activatable)
                        .map(|mana| {
                            (
                                permanent.object_id,
                                permanent.zone_change_generation,
                                mana.ability_index,
                                mana.mana_produced.clone(),
                            )
                        })
                })
                .find(|offer| kaya_output_has_color(&offer.3, 'U'))
                .expect("current untapped Island offers one blue mana");
            assert!(kaya_output_has_color(&output, 'U'));
            latest = tekuthal_normal_player_command(
                session,
                directory,
                rows,
                accepted_commands,
                TEKUTHAL_NORMAL_PILOT_ID,
                RuledCommand {
                    cmd: Some(Cmd::ActivateAbility(ActivateAbility {
                        source_object_id: object_id,
                        ability_index,
                        source_zone: AbilitySourceZone::Battlefield as i32,
                        expected_zone_change_generation: generation,
                        ..Default::default()
                    })),
                },
                battlefield,
            );
            produced.u += 1;
            let pool = &session
                .engine
                .as_ref()
                .expect("normal session has an engine while paying Bastion")
                .state
                .players
                .iter()
                .find(|player| player.id == TEKUTHAL_NORMAL_PILOT_ID)
                .expect("P17 remains in the normal session")
                .mana_pool;
            assert_eq!(pool.blue, produced.u);
            assert_eq!(pool.colorless, 0);
        }
        assert_eq!(produced.u, 4);

        let current_actions = kaya_batch(&latest)
            .legal_by_player
            .get(&TEKUTHAL_NORMAL_PILOT_ID)
            .expect("current Island response includes P17's legal actions");
        assert!(current_actions.pending_spell_cast.is_none());
        assert!(current_actions.pending_ability_activation.is_none());
        kaya_capture_battlefield(&latest, TEKUTHAL_NORMAL_PILOT_ID, battlefield);
        let bastion = battlefield
            .iter()
            .find(|permanent| permanent.object_id == 42 && permanent.card_id == bastion_card_id)
            .cloned()
            .expect("the current public battlefield offer contains Bastion ObjectId 42");
        assert!(
            !bastion.tapped,
            "Bastion remains ready while Islands produce mana"
        );
        let proliferate_index = registry
            .get(&bastion_card_id)
            .expect("actual Karn's Bastion definition is globally registered")
            .primary_face()
            .activated_abilities
            .iter()
            .position(|ability| ability.ability_id.as_str() == "activated_02")
            .expect("activated_02 is Karn's actual Proliferate ability");
        let ability = bastion
            .activated_abilities
            .iter()
            .find(|ability| ability.ability_index as usize == proliferate_index)
            .cloned()
            .expect("current Bastion offer publishes its actual Proliferate slot");
        assert!(ability.activatable);
        assert!(
            !ability.is_mana_ability,
            "select the Proliferate slot, not mana"
        );
        assert_eq!(ability.mana_cost, "{4}");
        let mut activation = ActivateAbility {
            source_object_id: bastion.object_id,
            ability_index: ability.ability_index,
            source_zone: AbilitySourceZone::Battlefield as i32,
            expected_zone_change_generation: bastion.zone_change_generation,
            ..Default::default()
        };
        let mut preview_correlation = u64::try_from(rows.len())
            .expect("timeline length fits payment preview correlation")
            .checked_add(1)
            .expect("payment preview correlation fits u64");
        let initial_request = IpcEnvelope {
            msg: Some(Msg::PaymentQuery(PaymentQuery {
                player_id: TEKUTHAL_NORMAL_PILOT_ID,
                preview: Some(PreviewPayment {
                    transaction_id: preview_correlation,
                    revision: preview_correlation,
                    activate_ability: Some(activation.clone()),
                    ..Default::default()
                }),
            })),
        };
        let initial_response = session
            .process(&initial_request)
            .expect("initial Bastion PaymentQuery keeps the normal session open");
        pair(directory, rows, &initial_request, &initial_response);
        assert!(
            initial_response.ok,
            "initial Bastion PaymentQuery: {}",
            initial_response.error
        );
        let initial_preview = initial_response
            .batch
            .as_ref()
            .and_then(|batch| batch.payment_preview.as_ref())
            .expect("initial Bastion PaymentQuery returns the current payment preview");
        assert!(
            initial_preview.valid,
            "Bastion preview: {initial_preview:?}"
        );
        assert_eq!(initial_preview.total_cost, "{4}");
        let mut proposed_payment = initial_preview
            .selection
            .clone()
            .expect("initial Bastion preview returns its current state revision");
        let expected_state_revision = proposed_payment.expected_state_revision;
        proposed_payment.mana = Some(produced);
        activation.payment = Some(proposed_payment);
        activation.restricted_mana = initial_preview.restricted_mana.clone();

        preview_correlation = preview_correlation
            .checked_add(1)
            .expect("payment preview correlation fits u64");
        let payment_request = IpcEnvelope {
            msg: Some(Msg::PaymentQuery(PaymentQuery {
                player_id: TEKUTHAL_NORMAL_PILOT_ID,
                preview: Some(PreviewPayment {
                    transaction_id: preview_correlation,
                    revision: preview_correlation,
                    activate_ability: Some(activation.clone()),
                    ..Default::default()
                }),
            })),
        };
        let payment_response = session
            .process(&payment_request)
            .expect("selected Bastion PaymentQuery keeps the normal session open");
        pair(directory, rows, &payment_request, &payment_response);
        assert!(
            payment_response.ok,
            "selected Bastion PaymentQuery: {}",
            payment_response.error
        );
        let preview = payment_response
            .batch
            .as_ref()
            .and_then(|batch| batch.payment_preview.as_ref())
            .expect("selected Bastion PaymentQuery returns its current preview");
        assert!(
            preview.valid && preview.complete,
            "Bastion preview: {preview:?}"
        );
        assert_eq!(preview.total_cost, "{4}");
        let selection = preview
            .selection
            .as_ref()
            .expect("complete Bastion PaymentQuery returns its exact selection");
        assert_eq!(selection.expected_state_revision, expected_state_revision);
        assert_eq!(selection.mana.as_ref(), Some(&produced));
        activation.payment = preview.selection.clone();
        activation.restricted_mana = preview.restricted_mana.clone();
        latest = tekuthal_normal_player_command(
            session,
            directory,
            rows,
            accepted_commands,
            TEKUTHAL_NORMAL_PILOT_ID,
            RuledCommand {
                cmd: Some(Cmd::ActivateAbility(activation)),
            },
            battlefield,
        );
        let stack_item_id = {
            let state = &session
                .engine
                .as_ref()
                .expect("normal session has an engine after Bastion payment")
                .state;
            assert!(state.pending_ability_activation.is_none());
            assert!(state.objects[&42].tapped, "Bastion's tap cost is paid");
            state
                .stack
                .last()
                .expect("committed Bastion activation is on the stack")
                .id
        };

        let mut proliferate_offer = None;
        for _ in 0..8 {
            proliferate_offer = kaya_choice(&latest, ChoiceKind::Proliferate);
            if proliferate_offer.is_some() {
                break;
            }
            let priority_player = session
                .engine
                .as_ref()
                .expect("normal session has an engine while Bastion is on the stack")
                .state
                .priority_player_id();
            latest = tekuthal_normal_player_command(
                session,
                directory,
                rows,
                accepted_commands,
                priority_player,
                RuledCommand {
                    cmd: Some(Cmd::PassPriority(PassPriority {})),
                },
                battlefield,
            );
        }
        let first_offer = proliferate_offer
            .expect("actual Bastion resolution publishes its first Proliferate prompt");
        assert_ne!(first_offer.resolution_prompt_id, 0);
        assert_eq!(first_offer.deciding_player_id, TEKUTHAL_NORMAL_PILOT_ID);
        assert_eq!(first_offer.min, 0);
        assert_eq!(first_offer.max, 2);
        assert!(first_offer.candidate_player_ids.is_empty());
        assert_eq!(first_offer.candidate_object_ids.len(), 2);
        assert!(first_offer.candidate_object_ids.contains(&36));
        assert!(first_offer.candidate_object_ids.contains(&41));

        latest = tekuthal_normal_player_command(
            session,
            directory,
            rows,
            accepted_commands,
            TEKUTHAL_NORMAL_PILOT_ID,
            RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    resolution_prompt_id: first_offer.resolution_prompt_id,
                    chosen_object_ids: vec![41],
                    chosen_player_ids: Vec::new(),
                    decision: ResolutionChoiceDecision::Unspecified as i32,
                    ..Default::default()
                })),
            },
            battlefield,
        );
        assert_eq!(
            session.engine.as_ref().unwrap().state.objects[&41]
                .counter_count(tricerules_cards::CounterKind::Night),
            3,
            "the first actual Proliferate adds Night to Ring only"
        );
        assert_eq!(
            session.engine.as_ref().unwrap().state.objects[&36]
                .counter_count(tricerules_cards::CounterKind::Charge),
            1
        );
        let second_offer = kaya_choice(&latest, ChoiceKind::Proliferate)
            .expect("the first choice immediately publishes the second Proliferate prompt");
        assert_ne!(
            second_offer.resolution_prompt_id,
            first_offer.resolution_prompt_id
        );
        assert_eq!(second_offer.deciding_player_id, TEKUTHAL_NORMAL_PILOT_ID);
        assert_eq!(second_offer.min, 0);
        assert_eq!(second_offer.max, 2);
        assert!(second_offer.candidate_player_ids.is_empty());
        assert_eq!(second_offer.candidate_object_ids.len(), 2);
        assert!(second_offer.candidate_object_ids.contains(&36));
        assert!(second_offer.candidate_object_ids.contains(&41));

        let accepted_commands_before_second_choice = *accepted_commands;
        latest = tekuthal_normal_player_command(
            session,
            directory,
            rows,
            accepted_commands,
            TEKUTHAL_NORMAL_PILOT_ID,
            RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    resolution_prompt_id: second_offer.resolution_prompt_id,
                    chosen_object_ids: vec![36],
                    chosen_player_ids: Vec::new(),
                    decision: ResolutionChoiceDecision::Unspecified as i32,
                    ..Default::default()
                })),
            },
            battlefield,
        );
        assert_eq!(
            *accepted_commands,
            accepted_commands_before_second_choice + 1,
            "the second fresh choice follows immediately, with no priority pass"
        );
        assert!(kaya_choice(&latest, ChoiceKind::Proliferate).is_none());
        let state = &session
            .engine
            .as_ref()
            .expect("normal session has an engine after both Proliferate choices")
            .state;
        assert!(
            state.stack.is_empty(),
            "both actual proliferates finish Bastion"
        );
        assert!(state.pending_resolution.is_none());
        assert!(state.pending_ability_activation.is_none());
        assert_eq!(
            state.players[state.active_player_idx].id,
            TEKUTHAL_NORMAL_PILOT_ID
        );
        assert_eq!(state.turn_step, tricerules_core::TurnStep::Main1);
        assert_eq!(state.priority_player_id(), TEKUTHAL_NORMAL_PILOT_ID);
        assert!(state.objects[&42].tapped, "Bastion remains tapped");
        assert!(!state.objects[&41].tapped, "Ring remains untapped");
        assert_eq!(
            state.objects[&41].counter_count(tricerules_cards::CounterKind::Night),
            3
        );
        assert_eq!(
            state.objects[&36].counter_count(tricerules_cards::CounterKind::Charge),
            2
        );
        assert_eq!(
            state
                .players
                .iter()
                .find(|player| player.id == TEKUTHAL_NORMAL_PILOT_ID)
                .unwrap()
                .life,
            20
        );
        assert!(kaya_batch(&latest)
            .legal_by_player
            .get(&TEKUTHAL_NORMAL_PILOT_ID)
            .expect("final current T5 response includes P17's actions")
            .pending_ability_activation
            .is_none());
        assert!(kaya_batch(&latest).events.iter().any(|event| {
            matches!(
                event.ev.as_ref(),
                Some(Ev::StackResolved(resolved)) if resolved.object_id == stack_item_id
            )
        }));
        latest
    }

    fn tekuthal_normal_counter_contribution(
        group: &tricerules_proto::ruled::v1::CounterRemovalGroupChoices,
        permanent: &BattlefieldObject,
        option_label: &str,
        count: u32,
    ) -> CounterRemovalContribution {
        let candidate = group
            .candidates
            .iter()
            .find(|candidate| {
                candidate.object.as_ref().is_some_and(|receipt| {
                    receipt.object_id == permanent.object_id
                        && receipt.zone_change_generation == permanent.zone_change_generation
                })
            })
            .expect("current group offer includes the exact permanent incarnation");
        let option = candidate
            .options
            .iter()
            .find(|option| {
                option.label.eq_ignore_ascii_case(option_label) && option.available_count >= count
            })
            .unwrap_or_else(|| {
                panic!(
                    "current group offer has {count} {option_label} counters on ObjectId {}",
                    permanent.object_id
                )
            });
        CounterRemovalContribution {
            object: candidate.object,
            option_id: option.option_id,
            count,
        }
    }

    fn tekuthal_normal_activate_tekuthal(
        session: &mut EngineSession,
        directory: &Path,
        rows: &mut Vec<String>,
        accepted_commands: &mut usize,
        mut latest: IpcResponse,
        battlefield: &mut Vec<BattlefieldObject>,
    ) -> IpcResponse {
        let registry = tricerules_cards::registry::global();
        let tekuthal_card_id = registry
            .id_for_name("Tekuthal, Inquiry Dominus")
            .expect("Tekuthal is in the global registry")
            .to_owned();
        let ring_card_id = registry
            .id_for_name("Replicating Ring")
            .expect("Replicating Ring is in the global registry")
            .to_owned();
        let chalice_card_id = registry
            .id_for_name("Everflowing Chalice")
            .expect("Everflowing Chalice is in the global registry")
            .to_owned();

        kaya_capture_battlefield(&latest, TEKUTHAL_NORMAL_PILOT_ID, battlefield);
        let current_actions = kaya_batch(&latest)
            .legal_by_player
            .get(&TEKUTHAL_NORMAL_PILOT_ID)
            .expect("current post-Bastion response includes P17's legal actions");
        assert!(current_actions.pending_ability_activation.is_none());
        assert!(current_actions.pending_spell_cast.is_none());
        let tekuthal = battlefield
            .iter()
            .find(|permanent| permanent.object_id == 33 && permanent.card_id == tekuthal_card_id)
            .cloned()
            .expect("current battlefield offer retains actual Tekuthal ObjectId 33");
        let activation_ability_index = registry
            .get(&tekuthal_card_id)
            .expect("actual Tekuthal definition is globally registered")
            .primary_face()
            .activated_abilities
            .iter()
            .position(|ability| ability.ability_id.as_str() == "activated_01")
            .expect("activated_01 is Tekuthal's actual activated ability");
        let ability = tekuthal
            .activated_abilities
            .iter()
            .find(|ability| ability.ability_index as usize == activation_ability_index)
            .cloned()
            .expect("current Tekuthal offer publishes activated_01");
        assert!(ability.activatable);
        assert!(!ability.is_mana_ability);
        let source = CostObjectRef {
            object_id: tekuthal.object_id,
            zone_change_generation: tekuthal.zone_change_generation,
        };
        let ability_key = (u64::from(source.object_id) << 32) | u64::from(ability.ability_index);
        let legal_costs = current_actions
            .cost_choices_by_ability
            .get(&ability_key)
            .expect("current Tekuthal offer publishes its grouped counter-removal choices")
            .clone();
        assert!(legal_costs.non_mana_costs_payable);
        let (cost_index, counter_group) = legal_costs
            .choices
            .iter()
            .find_map(|choice| {
                choice
                    .counter_removal_group
                    .as_ref()
                    .map(|group| (choice.cost_index, group.clone()))
            })
            .expect("current Tekuthal offer contains its aggregate counter-removal group");
        assert_eq!(counter_group.required_count, 3);
        assert!(
            !counter_group.candidates.iter().any(|candidate| {
                candidate
                    .object
                    .as_ref()
                    .is_some_and(|receipt| receipt.object_id == source.object_id)
            }),
            "Tekuthal is excluded by the current counter-removal offer"
        );
        let ring = battlefield
            .iter()
            .find(|permanent| permanent.object_id == 41 && permanent.card_id == ring_card_id)
            .cloned()
            .expect("current battlefield offer retains Ring ObjectId 41");
        let chalice = battlefield
            .iter()
            .find(|permanent| permanent.object_id == 36 && permanent.card_id == chalice_card_id)
            .cloned()
            .expect("current battlefield offer retains Chalice ObjectId 36");
        assert!(!ring.tapped);
        assert_eq!(
            session.engine.as_ref().unwrap().state.objects[&41]
                .counter_count(tricerules_cards::CounterKind::Night),
            3
        );
        assert_eq!(
            session.engine.as_ref().unwrap().state.objects[&36]
                .counter_count(tricerules_cards::CounterKind::Charge),
            2
        );
        assert_eq!(
            session.engine.as_ref().unwrap().state.objects[&33].zone,
            tricerules_core::Zone::Battlefield
        );

        let cost_selections = vec![CostSelection {
            cost_index,
            selection: Some(
                tricerules_proto::ruled::v1::cost_selection::Selection::CounterRemovalGroup(
                    CounterRemovalGroupSelection {
                        contributions: vec![
                            tekuthal_normal_counter_contribution(&counter_group, &ring, "night", 1),
                            tekuthal_normal_counter_contribution(
                                &counter_group,
                                &chalice,
                                "charge",
                                2,
                            ),
                        ],
                    },
                ),
            ),
        }];
        let flex_payments = vec![
            FlexPipPayment {
                pip_index: 1,
                pay_life: true,
                mana_color: 0,
            },
            FlexPipPayment {
                pip_index: 2,
                pay_life: true,
                mana_color: 0,
            },
        ];
        latest = tekuthal_normal_player_command(
            session,
            directory,
            rows,
            accepted_commands,
            TEKUTHAL_NORMAL_PILOT_ID,
            RuledCommand {
                cmd: Some(Cmd::BeginAbilityActivation(BeginAbilityActivation {
                    source_object_id: source.object_id,
                    expected_zone_change_generation: source.zone_change_generation,
                    ability_index: ability.ability_index,
                    source_zone: AbilitySourceZone::Battlefield as i32,
                    cost_selections: cost_selections.clone(),
                    flex_payments: flex_payments.clone(),
                    ..Default::default()
                })),
            },
            battlefield,
        );
        let pending = kaya_batch(&latest)
            .legal_by_player
            .get(&TEKUTHAL_NORMAL_PILOT_ID)
            .and_then(|actions| actions.pending_ability_activation.as_ref())
            .cloned()
            .expect("accepted Tekuthal activation publishes its payment transaction");
        let transaction_id = pending.transaction_id;
        let transaction_revision = pending.revision;
        assert_ne!(transaction_id, 0);
        assert_eq!(pending.source_object_id, source.object_id);
        assert_eq!(
            pending.source_zone_change_generation,
            source.zone_change_generation
        );
        assert_eq!(pending.ability_index, ability.ability_index);
        assert_eq!(pending.cost_selections, cost_selections);
        assert_eq!(pending.flex_payments, flex_payments);
        {
            let state = &session.engine.as_ref().unwrap().state;
            assert_eq!(
                state
                    .players
                    .iter()
                    .find(|player| player.id == TEKUTHAL_NORMAL_PILOT_ID)
                    .unwrap()
                    .life,
                20,
                "Begin does not pay either Phyrexian pip"
            );
            assert_eq!(
                state.objects[&41].counter_count(tricerules_cards::CounterKind::Night),
                3,
                "Begin does not remove Ring's Night counter"
            );
            assert_eq!(
                state.objects[&36].counter_count(tricerules_cards::CounterKind::Charge),
                2,
                "Begin does not remove Chalice's Charge counters"
            );
            assert!(state.stack.is_empty());
        }

        kaya_capture_battlefield(&latest, TEKUTHAL_NORMAL_PILOT_ID, battlefield);
        let ring = battlefield
            .iter()
            .find(|permanent| permanent.object_id == 41 && permanent.card_id == ring_card_id)
            .cloned()
            .expect("current payment view retains Replicating Ring ObjectId 41");
        let ring_mana = ring
            .activated_abilities
            .iter()
            .find(|mana| mana.is_mana_ability && mana.activatable)
            .cloned()
            .expect("Ring publishes its current mana ability");
        assert!(!ring.tapped);
        let offered_option_labels = ring_mana
            .mana_produced
            .split('/')
            .map(str::trim)
            .collect::<Vec<_>>();
        assert_eq!(offered_option_labels, vec!["W", "U", "B", "R", "G"]);
        assert_eq!(offered_option_labels.len(), 5);
        let offered_colors = offered_option_labels
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>();
        let expected_colors = vec!["W", "U", "B", "R", "G"]
            .into_iter()
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(offered_colors, expected_colors);
        assert!(!offered_colors.contains("C"));
        let mana_option_index = u32::try_from(
            offered_option_labels
                .iter()
                .position(|option| *option == "U")
                .expect("current Ring mana offer includes blue"),
        )
        .expect("current Ring option index fits u32");
        latest = tekuthal_normal_player_command(
            session,
            directory,
            rows,
            accepted_commands,
            TEKUTHAL_NORMAL_PILOT_ID,
            RuledCommand {
                cmd: Some(Cmd::ActivateAbility(ActivateAbility {
                    source_object_id: ring.object_id,
                    ability_index: ring_mana.ability_index,
                    mana_option_index,
                    source_zone: AbilitySourceZone::Battlefield as i32,
                    expected_zone_change_generation: ring.zone_change_generation,
                    ..Default::default()
                })),
            },
            battlefield,
        );
        let pool = &session
            .engine
            .as_ref()
            .unwrap()
            .state
            .players
            .iter()
            .find(|player| player.id == TEKUTHAL_NORMAL_PILOT_ID)
            .unwrap()
            .mana_pool;
        assert_eq!(pool.blue, 1, "Ring's current U option pays generic mana");
        assert_eq!(pool.colorless, 0, "Ring has no colorless option");

        let pending_after_mana = kaya_batch(&latest)
            .legal_by_player
            .get(&TEKUTHAL_NORMAL_PILOT_ID)
            .and_then(|actions| actions.pending_ability_activation.as_ref())
            .cloned()
            .expect("Ring's mana activation preserves Tekuthal's payment transaction");
        assert_eq!(pending_after_mana.transaction_id, transaction_id);
        assert_eq!(pending_after_mana.revision, transaction_revision);
        assert_eq!(pending_after_mana.source_object_id, source.object_id);
        assert_eq!(
            pending_after_mana.source_zone_change_generation,
            source.zone_change_generation
        );
        assert_eq!(pending_after_mana.ability_index, ability.ability_index);
        assert_eq!(pending_after_mana.cost_selections, cost_selections);
        assert_eq!(pending_after_mana.flex_payments, flex_payments);
        let payment = pending_after_mana
            .payment_preview
            .as_ref()
            .expect("current Tekuthal transaction publishes its generic payment");
        assert_eq!(payment.total_cost, "{1} + 4 life");
        let expected_state_revision = session.engine.as_ref().unwrap().state.command_index;
        let proposed_commit = CommitAbilityActivation {
            transaction_id,
            expected_revision: transaction_revision,
            flex_payments: Vec::new(),
            payment: Some(PaymentSelection {
                expected_state_revision,
                source: Some(source),
                mana: Some(PaymentMana {
                    u: 1,
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        };
        let payment_request = IpcEnvelope {
            msg: Some(Msg::PaymentQuery(PaymentQuery {
                player_id: TEKUTHAL_NORMAL_PILOT_ID,
                preview: Some(PreviewPayment {
                    transaction_id,
                    revision: payment.revision,
                    commit_ability_activation: Some(proposed_commit),
                    ..Default::default()
                }),
            })),
        };
        let payment_response = session
            .process(&payment_request)
            .expect("Tekuthal PaymentQuery keeps the normal session open");
        pair(directory, rows, &payment_request, &payment_response);
        assert!(
            payment_response.ok,
            "Tekuthal PaymentQuery: {}",
            payment_response.error
        );
        let preview = payment_response
            .batch
            .as_ref()
            .and_then(|batch| batch.payment_preview.as_ref())
            .expect("Tekuthal PaymentQuery returns the current payment preview");
        assert!(
            preview.valid && preview.complete,
            "Tekuthal payment: {preview:?}"
        );
        assert_eq!(preview.transaction_id, transaction_id);
        assert_eq!(preview.total_cost, "{1} + 4 life");
        let selection = preview
            .selection
            .clone()
            .expect("complete Tekuthal PaymentQuery returns its exact selection");
        assert_eq!(selection.expected_state_revision, expected_state_revision);
        assert_eq!(selection.source.as_ref(), Some(&source));
        assert_eq!(
            selection.mana.as_ref(),
            Some(&PaymentMana {
                u: 1,
                ..Default::default()
            })
        );
        let restricted_mana = preview.restricted_mana.clone();
        latest = tekuthal_normal_player_command(
            session,
            directory,
            rows,
            accepted_commands,
            TEKUTHAL_NORMAL_PILOT_ID,
            RuledCommand {
                cmd: Some(Cmd::CommitAbilityActivation(CommitAbilityActivation {
                    transaction_id,
                    expected_revision: transaction_revision,
                    flex_payments: Vec::new(),
                    payment: Some(selection),
                    restricted_mana,
                    ..Default::default()
                })),
            },
            battlefield,
        );
        let stack_item_id = {
            let state = &session.engine.as_ref().unwrap().state;
            assert!(state.pending_ability_activation.is_none());
            assert_eq!(
                state
                    .players
                    .iter()
                    .find(|player| player.id == TEKUTHAL_NORMAL_PILOT_ID)
                    .unwrap()
                    .life,
                16
            );
            assert_eq!(
                state.objects[&41].counter_count(tricerules_cards::CounterKind::Night),
                2
            );
            assert_eq!(
                state.objects[&36].counter_count(tricerules_cards::CounterKind::Charge),
                0
            );
            assert!(state.objects[&41].tapped);
            assert_eq!(
                state
                    .players
                    .iter()
                    .find(|player| player.id == TEKUTHAL_NORMAL_PILOT_ID)
                    .unwrap()
                    .mana_pool
                    .blue,
                0
            );
            state
                .stack
                .last()
                .expect("committed Tekuthal activation is on the stack")
                .id
        };
        latest = tekuthal_normal_resolve_stack(
            session,
            directory,
            rows,
            accepted_commands,
            latest,
            battlefield,
        );
        let state = &session.engine.as_ref().unwrap().state;
        let indestructible =
            tricerules_cards::CounterKind::Keyword(tricerules_cards::Keyword::Indestructible);
        assert_eq!(state.turn_step, tricerules_core::TurnStep::Main1);
        assert_eq!(
            state.players[state.active_player_idx].id,
            TEKUTHAL_NORMAL_PILOT_ID
        );
        assert_eq!(state.priority_player_id(), TEKUTHAL_NORMAL_PILOT_ID);
        assert!(state.stack.is_empty());
        assert!(state.pending_resolution.is_none());
        assert!(state.pending_ability_activation.is_none());
        assert!(state.pending_spell_cast.is_none());
        assert_eq!(state.objects[&33].zone, tricerules_core::Zone::Battlefield);
        assert_eq!(
            state.zone_change_generation.get(&source.object_id),
            Some(&source.zone_change_generation)
        );
        assert_eq!(state.objects[&33].counter_count(indestructible), 1);
        assert_eq!(
            state.objects[&41].counter_count(tricerules_cards::CounterKind::Night),
            2
        );
        assert_eq!(
            state.objects[&36].counter_count(tricerules_cards::CounterKind::Charge),
            0
        );
        assert!(state.objects[&41].tapped, "Ring supplied the U payment");
        assert!(state.objects[&42].tapped, "Bastion remains tapped");
        let pilot = state
            .players
            .iter()
            .find(|player| player.id == TEKUTHAL_NORMAL_PILOT_ID)
            .expect("P17 remains in the normal session");
        assert_eq!(pilot.life, 16);
        assert_eq!(pilot.mana_pool.blue, 0);
        assert_eq!(pilot.mana_pool.colorless, 0);
        assert!(
            state
                .players
                .iter()
                .find(|player| player.id == TEKUTHAL_NORMAL_OPPONENT_ID)
                .unwrap()
                .hand
                .contains(&91),
            "P29's Murder remains in hand"
        );
        let tekuthal_view = battlefield
            .iter()
            .find(|permanent| permanent.object_id == source.object_id)
            .expect("current battlefield view retains Tekuthal's same incarnation");
        assert_eq!(tekuthal_view.card_id, tekuthal_card_id);
        assert_eq!(
            tekuthal_view.zone_change_generation,
            source.zone_change_generation
        );
        assert!(tekuthal_view
            .keywords
            .iter()
            .any(|keyword| keyword == "Indestructible"));
        assert!(kaya_batch(&latest).events.iter().any(|event| {
            matches!(
                event.ev.as_ref(),
                Some(Ev::StackResolved(resolved)) if resolved.object_id == stack_item_id
            )
        }));
        latest
    }

    fn tekuthal_normal_battlefield_count(
        session: &EngineSession,
        player_id: i32,
        card_name: &str,
    ) -> usize {
        let card_id = tricerules_cards::registry::global()
            .id_for_name(card_name)
            .expect("battlefield card name resolves through global registry")
            .to_owned();
        let state = &session
            .engine
            .as_ref()
            .expect("normal Tekuthal session has an engine")
            .state;
        let player = state
            .players
            .iter()
            .find(|player| player.id == player_id)
            .expect("player remains in normal session");
        player
            .battlefield
            .iter()
            .filter_map(|object_id| state.objects.get(object_id))
            .filter(|object| object.card_id == card_id)
            .count()
    }

    fn tekuthal_normal_cast_murder(
        session: &mut EngineSession,
        directory: &Path,
        rows: &mut Vec<String>,
        accepted_commands: &mut usize,
        mut latest: IpcResponse,
        pilot_battlefield: &mut Vec<BattlefieldObject>,
    ) -> IpcResponse {
        let registry = tricerules_cards::registry::global();
        let tekuthal_card_id = registry
            .id_for_name("Tekuthal, Inquiry Dominus")
            .expect("Tekuthal is in the global registry")
            .to_owned();
        let murder_card_id = registry
            .id_for_name("Murder")
            .expect("Murder is in the global registry")
            .to_owned();
        let swamp_card_id = registry
            .id_for_name("Swamp")
            .expect("Swamp is in the global registry")
            .to_owned();
        let (turn_instance, tekuthal_generation) = {
            let state = &session
                .engine
                .as_ref()
                .expect("normal Tekuthal session has an engine before Murder")
                .state;
            assert_eq!(state.turn_step, tricerules_core::TurnStep::Main1);
            assert_eq!(
                state.players[state.active_player_idx].id,
                TEKUTHAL_NORMAL_PILOT_ID
            );
            assert_eq!(state.priority_player_id(), TEKUTHAL_NORMAL_PILOT_ID);
            assert!(state.stack.is_empty());
            assert!(state.pending_spell_cast.is_none());
            assert!(state.pending_resolution.is_none());
            assert!(state.pending_ability_activation.is_none());

            let tekuthal = state
                .objects
                .get(&33)
                .expect("physical Tekuthal ObjectId 33 remains in the game state");
            assert_eq!(tekuthal.card_id, tekuthal_card_id);
            assert_eq!(tekuthal.zone, tricerules_core::Zone::Battlefield);
            assert_eq!(
                tekuthal.counter_count(tricerules_cards::CounterKind::Keyword(
                    tricerules_cards::Keyword::Indestructible
                )),
                1
            );
            assert_eq!(
                state
                    .players
                    .iter()
                    .find(|player| player.id == TEKUTHAL_NORMAL_PILOT_ID)
                    .expect("P17 remains in the normal session")
                    .life,
                16
            );
            assert_eq!(
                state.objects[&41].counter_count(tricerules_cards::CounterKind::Night),
                2
            );
            assert_eq!(
                state.objects[&36].counter_count(tricerules_cards::CounterKind::Charge),
                0
            );
            assert!(state.objects[&41].tapped);
            (state.turn_instance, captured_generation(session, 33))
        };

        let mut opponent_battlefield = Vec::new();
        kaya_capture_battlefield(
            &latest,
            TEKUTHAL_NORMAL_OPPONENT_ID,
            &mut opponent_battlefield,
        );
        assert_eq!(
            opponent_battlefield
                .iter()
                .filter(|permanent| permanent.card_id == swamp_card_id && !permanent.tapped)
                .count(),
            4,
            "current P29 battlefield view includes its four untapped Swamps"
        );
        let tekuthal_receipt = pilot_battlefield
            .iter()
            .find(|permanent| permanent.object_id == 33 && permanent.card_id == tekuthal_card_id)
            .cloned()
            .expect("current P17 public view retains Tekuthal ObjectId 33");
        assert_eq!(tekuthal_receipt.zone_change_generation, tekuthal_generation);
        assert!(tekuthal_receipt
            .keywords
            .iter()
            .any(|keyword| keyword == "Indestructible"));

        if session
            .engine
            .as_ref()
            .expect("normal Tekuthal session has an engine before Murder priority")
            .state
            .priority_player_id()
            == TEKUTHAL_NORMAL_PILOT_ID
        {
            latest = tekuthal_normal_player_command(
                session,
                directory,
                rows,
                accepted_commands,
                TEKUTHAL_NORMAL_PILOT_ID,
                RuledCommand {
                    cmd: Some(Cmd::PassPriority(PassPriority {})),
                },
                pilot_battlefield,
            );
            kaya_capture_battlefield(
                &latest,
                TEKUTHAL_NORMAL_OPPONENT_ID,
                &mut opponent_battlefield,
            );
        }
        {
            let state = &session
                .engine
                .as_ref()
                .expect("normal Tekuthal session has an engine at P29 priority")
                .state;
            assert_eq!(state.turn_instance, turn_instance);
            assert_eq!(state.turn_step, tricerules_core::TurnStep::Main1);
            assert_eq!(
                state.players[state.active_player_idx].id,
                TEKUTHAL_NORMAL_PILOT_ID
            );
            assert_eq!(state.priority_player_id(), TEKUTHAL_NORMAL_OPPONENT_ID);
            assert!(state.stack.is_empty());
        }

        let opponent_actions = kaya_batch(&latest)
            .legal_by_player
            .get(&TEKUTHAL_NORMAL_OPPONENT_ID)
            .expect("current P29 response includes its legal hand actions");
        assert!(opponent_actions.pending_spell_cast.is_none());
        let murder = opponent_actions
            .hand_actions
            .iter()
            .find(|offer| {
                offer.kind == HandActionKind::HandActionCastSpell as i32
                    && offer.card_name == "Murder"
            })
            .cloned()
            .expect("current P29 offer contains the actual Murder spell");
        let murder_hand_index = session
            .engine
            .as_ref()
            .expect("normal Tekuthal session has an engine for P29's current hand")
            .state
            .players
            .iter()
            .find(|player| player.id == TEKUTHAL_NORMAL_OPPONENT_ID)
            .expect("P29 remains in the normal session")
            .hand
            .iter()
            .position(|object_id| *object_id == 91)
            .expect("physical Murder ObjectId 91 remains in P29's current hand");
        assert_eq!(
            session.engine.as_ref().unwrap().state.objects[&91].card_id,
            murder_card_id
        );
        assert_eq!(
            session.engine.as_ref().unwrap().state.objects[&91].zone,
            tricerules_core::Zone::Hand
        );
        assert_eq!(
            murder.hand_index,
            u32::try_from(murder_hand_index).expect("current hand slot fits the offer index")
        );
        let target_key = (murder.hand_index << 8) | murder.face_index;
        let target_groups = &opponent_actions
            .valid_targets_by_hand_slot
            .get(&target_key)
            .unwrap_or_else(|| {
                panic!("current Murder hand slot {target_key} publishes target groups")
            })
            .groups;
        assert_eq!(
            target_groups.len(),
            1,
            "current Murder offer has one target group"
        );
        let target_group = &target_groups[0];
        assert!(target_group
            .valid_permanent_ids
            .contains(&tekuthal_receipt.object_id));
        let target_ref = luxior_normal_target_ref(target_group, &tekuthal_receipt);

        latest = tekuthal_normal_player_command(
            session,
            directory,
            rows,
            accepted_commands,
            TEKUTHAL_NORMAL_OPPONENT_ID,
            RuledCommand {
                cmd: Some(Cmd::BeginSpellCast(BeginSpellCast {
                    announcement: Some(tricerules_proto::ruled::v1::SpellCastAnnouncement {
                        source: Some(CastSource {
                            location: Some(
                                tricerules_proto::ruled::v1::cast_source::Location::HandIndex(
                                    murder.hand_index,
                                ),
                            ),
                            expected_zone_change_generation: None,
                        }),
                        face_index: murder.face_index,
                        cast_method: CastMethod::Normal as i32,
                        targets: vec![target_ref],
                        ..Default::default()
                    }),
                })),
            },
            pilot_battlefield,
        );
        kaya_capture_battlefield(
            &latest,
            TEKUTHAL_NORMAL_OPPONENT_ID,
            &mut opponent_battlefield,
        );
        let pending = kaya_batch(&latest)
            .legal_by_player
            .get(&TEKUTHAL_NORMAL_OPPONENT_ID)
            .and_then(|actions| actions.pending_spell_cast.as_ref())
            .expect("current P29 response publishes Murder's pending payment");
        let initial_payment = pending
            .payment_preview
            .as_ref()
            .expect("pending Murder publishes the canonical payment preview");
        assert_eq!(initial_payment.total_cost, "{1}{B}{B}");
        let transaction_id = pending.transaction_id;

        let mut produced = PaymentMana::default();
        for _ in 0..3 {
            let (object_id, generation, ability_index, output) = opponent_battlefield
                .iter()
                .filter(|permanent| permanent.card_id == swamp_card_id && !permanent.tapped)
                .flat_map(|permanent| {
                    permanent
                        .activated_abilities
                        .iter()
                        .filter(|ability| ability.is_mana_ability && ability.activatable)
                        .map(|ability| {
                            (
                                permanent.object_id,
                                permanent.zone_change_generation,
                                ability.ability_index,
                                ability.mana_produced.clone(),
                            )
                        })
                })
                .find(|source| kaya_output_has_color(&source.3, 'B'))
                .expect("current untapped P29 Swamps offer black mana for Murder");
            assert!(kaya_output_has_color(&output, 'B'));
            latest = tekuthal_normal_player_command(
                session,
                directory,
                rows,
                accepted_commands,
                TEKUTHAL_NORMAL_OPPONENT_ID,
                RuledCommand {
                    cmd: Some(Cmd::ActivateAbility(ActivateAbility {
                        source_object_id: object_id,
                        ability_index,
                        source_zone: AbilitySourceZone::Battlefield as i32,
                        expected_zone_change_generation: generation,
                        ..Default::default()
                    })),
                },
                pilot_battlefield,
            );
            kaya_capture_battlefield(
                &latest,
                TEKUTHAL_NORMAL_OPPONENT_ID,
                &mut opponent_battlefield,
            );
            produced.b += 1;
            let pool = &session
                .engine
                .as_ref()
                .expect("normal Tekuthal session has an engine while paying Murder")
                .state
                .players
                .iter()
                .find(|player| player.id == TEKUTHAL_NORMAL_OPPONENT_ID)
                .expect("P29 remains in the normal session")
                .mana_pool;
            assert_eq!(pool.black, produced.b);
            assert_eq!(pool.colorless, 0);
        }
        assert_eq!(produced.b, 3);

        let pending = kaya_batch(&latest)
            .legal_by_player
            .get(&TEKUTHAL_NORMAL_OPPONENT_ID)
            .and_then(|actions| actions.pending_spell_cast.as_ref())
            .expect("Murder's payment transaction remains open after Swamp activations");
        assert_eq!(pending.transaction_id, transaction_id);
        let payment = pending
            .payment_preview
            .as_ref()
            .expect("current pending Murder keeps its payment preview");
        assert_eq!(payment.total_cost, "{1}{B}{B}");
        let revision = payment.revision;
        let expected_state_revision = session
            .engine
            .as_ref()
            .expect("normal session has an engine for Murder payment preview")
            .state
            .command_index;
        let payment_request = IpcEnvelope {
            msg: Some(Msg::PaymentQuery(PaymentQuery {
                player_id: TEKUTHAL_NORMAL_OPPONENT_ID,
                preview: Some(PreviewPayment {
                    transaction_id,
                    revision,
                    commit_spell_cast: Some(CommitSpellCast {
                        transaction_id,
                        payment: Some(PaymentSelection {
                            expected_state_revision,
                            mana: Some(produced),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
            })),
        };
        let payment_response = session
            .process(&payment_request)
            .expect("Murder PaymentQuery keeps the normal session open");
        pair(directory, rows, &payment_request, &payment_response);
        assert!(
            payment_response.ok,
            "Murder PaymentQuery: {}",
            payment_response.error
        );
        kaya_capture_battlefield(
            &payment_response,
            TEKUTHAL_NORMAL_OPPONENT_ID,
            &mut opponent_battlefield,
        );
        let preview = payment_response
            .batch
            .as_ref()
            .and_then(|batch| batch.payment_preview.as_ref())
            .expect("Murder PaymentQuery returns the current payment preview");
        assert!(
            preview.valid && preview.complete,
            "Murder payment: {preview:?}"
        );
        assert_eq!(preview.transaction_id, transaction_id);
        assert_eq!(preview.total_cost, "{1}{B}{B}");
        let selection = preview
            .selection
            .clone()
            .expect("complete Murder PaymentQuery returns its exact selection");
        assert_eq!(selection.expected_state_revision, expected_state_revision);
        assert_eq!(selection.mana.as_ref(), Some(&produced));
        let restricted_mana = preview.restricted_mana.clone();

        latest = tekuthal_normal_player_command(
            session,
            directory,
            rows,
            accepted_commands,
            TEKUTHAL_NORMAL_OPPONENT_ID,
            RuledCommand {
                cmd: Some(Cmd::CommitSpellCast(CommitSpellCast {
                    transaction_id,
                    payment: Some(selection),
                    restricted_mana,
                })),
            },
            pilot_battlefield,
        );
        kaya_capture_battlefield(
            &latest,
            TEKUTHAL_NORMAL_OPPONENT_ID,
            &mut opponent_battlefield,
        );
        assert!(session
            .engine
            .as_ref()
            .expect("normal session has an engine after Murder commit")
            .state
            .pending_spell_cast
            .is_none());
        assert_eq!(
            session.engine.as_ref().unwrap().state.objects[&91].zone,
            tricerules_core::Zone::Stack
        );

        latest = tekuthal_normal_resolve_stack(
            session,
            directory,
            rows,
            accepted_commands,
            latest,
            pilot_battlefield,
        );
        kaya_capture_battlefield(
            &latest,
            TEKUTHAL_NORMAL_OPPONENT_ID,
            &mut opponent_battlefield,
        );
        kaya_capture_battlefield(&latest, TEKUTHAL_NORMAL_PILOT_ID, pilot_battlefield);
        let state = &session
            .engine
            .as_ref()
            .expect("normal session has an engine after Murder resolves")
            .state;
        assert_eq!(state.turn_instance, turn_instance);
        assert_eq!(state.turn_step, tricerules_core::TurnStep::Main1);
        assert_eq!(
            state.players[state.active_player_idx].id,
            TEKUTHAL_NORMAL_PILOT_ID
        );
        assert_eq!(state.priority_player_id(), TEKUTHAL_NORMAL_PILOT_ID);
        assert!(state.stack.is_empty());
        assert!(state.pending_spell_cast.is_none());
        assert!(state.pending_resolution.is_none());
        assert!(state.pending_ability_activation.is_none());
        assert_eq!(state.objects[&91].card_id, murder_card_id);
        assert_eq!(state.objects[&91].zone, tricerules_core::Zone::Graveyard);
        assert_eq!(state.objects[&33].zone, tricerules_core::Zone::Battlefield);
        assert_eq!(captured_generation(session, 33), tekuthal_generation);
        assert_eq!(
            state.objects[&33].counter_count(tricerules_cards::CounterKind::Keyword(
                tricerules_cards::Keyword::Indestructible
            )),
            1
        );
        assert_eq!(
            state
                .players
                .iter()
                .find(|player| player.id == TEKUTHAL_NORMAL_PILOT_ID)
                .expect("P17 remains in the normal session after Murder")
                .life,
            16
        );
        assert_eq!(
            state.objects[&41].counter_count(tricerules_cards::CounterKind::Night),
            2
        );
        assert_eq!(
            state.objects[&36].counter_count(tricerules_cards::CounterKind::Charge),
            0
        );
        assert!(state.objects[&41].tapped);
        assert_eq!(
            state
                .players
                .iter()
                .find(|player| player.id == TEKUTHAL_NORMAL_OPPONENT_ID)
                .expect("P29 remains in the normal session after Murder")
                .mana_pool
                .black,
            0
        );
        let final_tekuthal_receipt = pilot_battlefield
            .iter()
            .find(|permanent| permanent.object_id == 33 && permanent.card_id == tekuthal_card_id)
            .expect("final P17 public view retains Tekuthal ObjectId 33");
        assert_eq!(
            final_tekuthal_receipt.zone_change_generation,
            tekuthal_generation
        );
        assert!(final_tekuthal_receipt
            .keywords
            .iter()
            .any(|keyword| keyword == "Indestructible"));
        assert!(kaya_batch(&latest)
            .legal_by_player
            .get(&TEKUTHAL_NORMAL_OPPONENT_ID)
            .expect("final response retains P29's current actions")
            .pending_spell_cast
            .is_none());
        println!(
            "T5 Murder resolved: P29 spent three current Swamp offers; physical Murder91 is in graveyard; same-generation Tekuthal33 remains with Indestructible1; P17 life=16, Ring Night=2, Chalice Charge=0; accepted PlayerCommands={accepted_commands}, timeline records={}",
            rows.len()
        );
        latest
    }

    #[test]
    fn tekuthal_normal_pilot_records_and_replays_grouped_activation() {
        let player_decks = tekuthal_normal_pilot_decks();
        tekuthal_normal_assert_decks(&player_decks);
        let validation_names = player_decks
            .iter()
            .flat_map(|deck| deck.mainboard_card_name.iter().cloned())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();

        let mut session = EngineSession::new(false);
        let validation = session
            .process(&IpcEnvelope {
                msg: Some(Msg::ValidateDeck(ValidateDeck {
                    card_names: validation_names,
                })),
            })
            .expect("ValidateDeck keeps the Tekuthal normal-pilot session open");
        assert!(
            validation.ok,
            "Tekuthal normal-pilot deck validation: {}",
            validation.error
        );
        assert!(validation.missing_card_names.is_empty());

        let root = tempfile::tempdir()
            .expect("unique normal-pilot capture directory")
            .keep();
        println!("retained_capture={}", root.display());
        fs::create_dir(root.join("raw")).expect("create raw capture directory");
        let mut rows = Vec::new();
        let mut manifest = serde_json::json!({
            "format_version": 1,
            "source": "server",
            "privacy": "server_only",
            "capture_id": root.as_path().file_name().unwrap().to_string_lossy().to_string(),
            "engine_build": ENGINE_BUILD,
            "card_data_hash": "",
            "effective_dev_commands_enabled": false,
            "complete": false
        });
        let start = IpcEnvelope {
            msg: Some(Msg::SessionStart(SessionStart {
                game_id: 7_281_036,
                seed: 3,
                player_ids: vec![TEKUTHAL_NORMAL_PILOT_ID, TEKUTHAL_NORMAL_OPPONENT_ID],
                player_decks,
                dev_commands_enabled: false,
                diagnostic_capture_enabled: true,
                ..Default::default()
            })),
        };
        let start_response = session
            .process(&start)
            .expect("strict normal SessionStart keeps the pilot open");
        assert!(
            start_response.ok,
            "normal Tekuthal GameStart: {}",
            start_response.error
        );
        assert!(!start_response.effective_dev_commands_enabled);
        manifest["card_data_hash"] = start_response.card_data_hash.clone().into();
        fs::write(root.as_path().join("manifest.json"), manifest.to_string())
            .expect("write initial capture manifest");
        pair(root.as_path(), &mut rows, &start, &start_response);

        let mut battlefield = Vec::new();
        kaya_capture_battlefield(&start_response, TEKUTHAL_NORMAL_PILOT_ID, &mut battlefield);
        let mut accepted_commands = 0usize;
        let starting = kaya_starting_player_offer(&start_response)
            .expect("GameStart publishes the current starting-player offer");
        assert!(starting
            .eligible_starting_player_ids
            .contains(&TEKUTHAL_NORMAL_PILOT_ID));
        let mut latest = tekuthal_normal_player_command(
            &mut session,
            root.as_path(),
            &mut rows,
            &mut accepted_commands,
            starting.deciding_player_id,
            RuledCommand {
                cmd: Some(Cmd::ChooseStartingPlayer(ChooseStartingPlayer {
                    starting_player_id: TEKUTHAL_NORMAL_PILOT_ID,
                })),
            },
            &mut battlefield,
        );
        for _ in 0..2 {
            let Some(opening) = kaya_mulligan_offer(&latest) else {
                break;
            };
            latest = tekuthal_normal_player_command(
                &mut session,
                root.as_path(),
                &mut rows,
                &mut accepted_commands,
                opening.deciding_player_id,
                RuledCommand {
                    cmd: Some(Cmd::Mulligan(MulliganDecision { keep: true })),
                },
                &mut battlefield,
            );
        }
        assert!(
            kaya_mulligan_offer(&latest).is_none(),
            "both initial hands are kept with ordinary decisions"
        );

        let (p17_opening, p17_draws, p29_opening) = {
            let engine = session
                .engine
                .as_ref()
                .expect("normal session has an engine");
            let state = &engine.state;
            assert_eq!(
                state.players[state.starting_player_idx].id,
                TEKUTHAL_NORMAL_PILOT_ID
            );
            let p17 = state
                .players
                .iter()
                .find(|player| player.id == TEKUTHAL_NORMAL_PILOT_ID)
                .expect("P17 exists in the game state");
            let p29 = state
                .players
                .iter()
                .find(|player| player.id == TEKUTHAL_NORMAL_OPPONENT_ID)
                .expect("P29 exists in the game state");
            (
                tekuthal_normal_card_records(state, p17.hand.iter().copied()),
                tekuthal_normal_card_records(state, p17.library.iter().take(4).copied()),
                tekuthal_normal_card_records(state, p29.hand.iter().copied()),
            )
        };
        let p17_opening_names = p17_opening
            .iter()
            .map(|(object_id, _, name)| (*object_id, name.clone()))
            .collect::<Vec<_>>();
        assert_eq!(
            p17_opening_names,
            vec![
                (41, "Replicating Ring".to_string()),
                (42, "Karn's Bastion".to_string()),
                (89, "Island".to_string()),
                (33, "Tekuthal, Inquiry Dominus".to_string()),
                (36, "Everflowing Chalice".to_string()),
                (58, "Island".to_string()),
                (64, "Island".to_string()),
            ]
        );
        assert_eq!(
            p17_draws
                .iter()
                .map(|(object_id, _, name)| (*object_id, name.clone()))
                .collect::<Vec<_>>(),
            vec![
                (48, "Island".to_string()),
                (37, "Everflowing Chalice".to_string()),
                (51, "Island".to_string()),
                (49, "Island".to_string())
            ]
        );
        assert_eq!(p29_opening.len(), 7);
        assert_eq!(
            p29_opening
                .iter()
                .filter(|(_, _, name)| name.as_str() == "Murder")
                .map(|(object_id, _, _)| *object_id)
                .collect::<Vec<_>>(),
            vec![91]
        );
        assert_eq!(
            p29_opening
                .iter()
                .filter(|(_, _, name)| name.as_str() == "Swamp")
                .count(),
            6
        );
        println!(
            "P17 exact deck order={:?}",
            tekuthal_normal_pilot_decks()[0].mainboard_card_name
        );
        println!(
            "P29 exact deck order={:?}",
            tekuthal_normal_pilot_decks()[1].mainboard_card_name
        );
        println!("P17 opening hand (ObjectId, global card id, registry name)={p17_opening:?}");
        println!("P17 scheduled draws T2-T5 from library front={p17_draws:?}");
        println!("P29 opening hand (ObjectId, global card id, registry name)={p29_opening:?}");

        for _ in 0..32 {
            let state = &session
                .engine
                .as_ref()
                .expect("normal session has engine")
                .state;
            if state.turn_step == tricerules_core::TurnStep::Main1
                && state.players[state.active_player_idx].id == TEKUTHAL_NORMAL_PILOT_ID
            {
                break;
            }
            let priority_player = state.priority_player_id();
            latest = tekuthal_normal_player_command(
                &mut session,
                root.as_path(),
                &mut rows,
                &mut accepted_commands,
                priority_player,
                RuledCommand {
                    cmd: Some(Cmd::PassPriority(PassPriority {})),
                },
                &mut battlefield,
            );
        }
        {
            let state = &session
                .engine
                .as_ref()
                .expect("normal session has engine")
                .state;
            assert_eq!(state.turn_step, tricerules_core::TurnStep::Main1);
            assert_eq!(
                state.players[state.active_player_idx].id,
                TEKUTHAL_NORMAL_PILOT_ID
            );
            assert_eq!(state.priority_player_id(), TEKUTHAL_NORMAL_PILOT_ID);
            assert_eq!(state.players[state.active_player_idx].hand.len(), 7);
        }

        let island = kaya_batch(&latest)
            .legal_by_player
            .get(&TEKUTHAL_NORMAL_PILOT_ID)
            .expect("P17 receives current T1 hand actions")
            .hand_actions
            .iter()
            .find(|offer| {
                offer.kind == HandActionKind::HandActionPlayLand as i32
                    && offer.card_name == "Island"
            })
            .cloned()
            .expect("current T1 hand offer includes an Island");
        latest = tekuthal_normal_player_command(
            &mut session,
            root.as_path(),
            &mut rows,
            &mut accepted_commands,
            TEKUTHAL_NORMAL_PILOT_ID,
            RuledCommand {
                cmd: Some(Cmd::PlayLand(PlayLand {
                    source: Some(LandSource {
                        location: Some(
                            tricerules_proto::ruled::v1::land_source::Location::HandIndex(
                                island.hand_index,
                            ),
                        ),
                        ..Default::default()
                    }),
                    ..Default::default()
                })),
            },
            &mut battlefield,
        );
        {
            let state = &session
                .engine
                .as_ref()
                .expect("normal session has engine")
                .state;
            assert_eq!(
                state.turn_step,
                tricerules_core::TurnStep::Main1,
                "legal T1 land play leaves the active main phase open"
            );
            assert_eq!(
                state.players[state.active_player_idx].id,
                TEKUTHAL_NORMAL_PILOT_ID
            );
            let island_id = tricerules_cards::registry::global()
                .id_for_name("Island")
                .expect("Island is in the global registry");
            assert_eq!(
                state.players[state.active_player_idx]
                    .battlefield
                    .iter()
                    .filter(|object_id| state.objects[*object_id].card_id == island_id)
                    .count(),
                1
            );
        }
        println!(
            "T1 legal Island played; accepted PlayerCommands={accepted_commands}; phase={:?}",
            session.engine.as_ref().unwrap().state.turn_step
        );

        latest = tekuthal_normal_advance_to_main(
            &mut session,
            root.as_path(),
            &mut rows,
            &mut accepted_commands,
            latest,
            &mut battlefield,
            TEKUTHAL_NORMAL_OPPONENT_ID,
        );
        latest = tekuthal_normal_play_land(
            &mut session,
            root.as_path(),
            &mut rows,
            &mut accepted_commands,
            &latest,
            &mut battlefield,
            TekuthalNormalLandPlan {
                player_id: TEKUTHAL_NORMAL_OPPONENT_ID,
                card_name: "Swamp",
            },
        );
        assert_eq!(
            tekuthal_normal_battlefield_count(&session, TEKUTHAL_NORMAL_OPPONENT_ID, "Swamp"),
            1
        );

        latest = tekuthal_normal_advance_to_main(
            &mut session,
            root.as_path(),
            &mut rows,
            &mut accepted_commands,
            latest,
            &mut battlefield,
            TEKUTHAL_NORMAL_PILOT_ID,
        );
        assert!(
            session
                .engine
                .as_ref()
                .unwrap()
                .state
                .players
                .iter()
                .find(|player| player.id == TEKUTHAL_NORMAL_PILOT_ID)
                .unwrap()
                .hand
                .contains(&48),
            "P17's real T2 draw is ObjectId 48 Island"
        );
        latest = tekuthal_normal_play_land(
            &mut session,
            root.as_path(),
            &mut rows,
            &mut accepted_commands,
            &latest,
            &mut battlefield,
            TekuthalNormalLandPlan {
                player_id: TEKUTHAL_NORMAL_PILOT_ID,
                card_name: "Island",
            },
        );
        assert_eq!(
            tekuthal_normal_battlefield_count(&session, TEKUTHAL_NORMAL_PILOT_ID, "Island"),
            2
        );
        latest = tekuthal_normal_cast_from_current_offer(
            &mut session,
            root.as_path(),
            &mut rows,
            &mut accepted_commands,
            latest,
            &mut battlefield,
            TekuthalNormalCastPlan {
                card_name: "Everflowing Chalice",
                expected_total_cost: "{2}",
                blue_mana: 2,
                kick_once: true,
            },
        );
        assert_eq!(
            session.engine.as_ref().unwrap().state.objects[&36].zone,
            tricerules_core::Zone::Battlefield
        );
        assert_eq!(
            session.engine.as_ref().unwrap().state.objects[&36]
                .counter_count(tricerules_cards::CounterKind::Charge),
            1
        );
        println!(
            "T2: Island played; once-kicked Chalice ObjectId=36 entered with one Charge counter"
        );

        latest = tekuthal_normal_advance_to_main(
            &mut session,
            root.as_path(),
            &mut rows,
            &mut accepted_commands,
            latest,
            &mut battlefield,
            TEKUTHAL_NORMAL_OPPONENT_ID,
        );
        latest = tekuthal_normal_play_land(
            &mut session,
            root.as_path(),
            &mut rows,
            &mut accepted_commands,
            &latest,
            &mut battlefield,
            TekuthalNormalLandPlan {
                player_id: TEKUTHAL_NORMAL_OPPONENT_ID,
                card_name: "Swamp",
            },
        );
        assert_eq!(
            tekuthal_normal_battlefield_count(&session, TEKUTHAL_NORMAL_OPPONENT_ID, "Swamp"),
            2
        );

        latest = tekuthal_normal_advance_to_main(
            &mut session,
            root.as_path(),
            &mut rows,
            &mut accepted_commands,
            latest,
            &mut battlefield,
            TEKUTHAL_NORMAL_PILOT_ID,
        );
        assert!(
            session
                .engine
                .as_ref()
                .unwrap()
                .state
                .players
                .iter()
                .find(|player| player.id == TEKUTHAL_NORMAL_PILOT_ID)
                .unwrap()
                .hand
                .contains(&37),
            "P17's real T3 draw is ObjectId 37 Everflowing Chalice"
        );
        latest = tekuthal_normal_play_land(
            &mut session,
            root.as_path(),
            &mut rows,
            &mut accepted_commands,
            &latest,
            &mut battlefield,
            TekuthalNormalLandPlan {
                player_id: TEKUTHAL_NORMAL_PILOT_ID,
                card_name: "Island",
            },
        );
        assert_eq!(
            tekuthal_normal_battlefield_count(&session, TEKUTHAL_NORMAL_PILOT_ID, "Island"),
            3
        );
        latest = tekuthal_normal_cast_from_current_offer(
            &mut session,
            root.as_path(),
            &mut rows,
            &mut accepted_commands,
            latest,
            &mut battlefield,
            TekuthalNormalCastPlan {
                card_name: "Replicating Ring",
                expected_total_cost: "{3}",
                blue_mana: 3,
                kick_once: false,
            },
        );
        assert_eq!(
            session.engine.as_ref().unwrap().state.objects[&41].zone,
            tricerules_core::Zone::Battlefield
        );
        println!("T3: Island and Replicating Ring ObjectId=41 played through current offers");

        latest = tekuthal_normal_advance_to_main(
            &mut session,
            root.as_path(),
            &mut rows,
            &mut accepted_commands,
            latest,
            &mut battlefield,
            TEKUTHAL_NORMAL_OPPONENT_ID,
        );
        latest = tekuthal_normal_play_land(
            &mut session,
            root.as_path(),
            &mut rows,
            &mut accepted_commands,
            &latest,
            &mut battlefield,
            TekuthalNormalLandPlan {
                player_id: TEKUTHAL_NORMAL_OPPONENT_ID,
                card_name: "Swamp",
            },
        );
        assert_eq!(
            tekuthal_normal_battlefield_count(&session, TEKUTHAL_NORMAL_OPPONENT_ID, "Swamp"),
            3
        );

        latest = tekuthal_normal_advance_to_main(
            &mut session,
            root.as_path(),
            &mut rows,
            &mut accepted_commands,
            latest,
            &mut battlefield,
            TEKUTHAL_NORMAL_PILOT_ID,
        );
        assert!(
            session
                .engine
                .as_ref()
                .unwrap()
                .state
                .players
                .iter()
                .find(|player| player.id == TEKUTHAL_NORMAL_PILOT_ID)
                .unwrap()
                .hand
                .contains(&51),
            "P17's real T4 draw is ObjectId 51 Island"
        );
        latest = tekuthal_normal_play_land(
            &mut session,
            root.as_path(),
            &mut rows,
            &mut accepted_commands,
            &latest,
            &mut battlefield,
            TekuthalNormalLandPlan {
                player_id: TEKUTHAL_NORMAL_PILOT_ID,
                card_name: "Island",
            },
        );
        assert_eq!(
            tekuthal_normal_battlefield_count(&session, TEKUTHAL_NORMAL_PILOT_ID, "Island"),
            4
        );
        assert_eq!(
            session.engine.as_ref().unwrap().state.objects[&41]
                .counter_count(tricerules_cards::CounterKind::Night),
            1,
            "Ring's ordinary T4 upkeep adds one Night counter before Tekuthal"
        );
        latest = tekuthal_normal_cast_from_current_offer(
            &mut session,
            root.as_path(),
            &mut rows,
            &mut accepted_commands,
            latest,
            &mut battlefield,
            TekuthalNormalCastPlan {
                card_name: "Tekuthal, Inquiry Dominus",
                expected_total_cost: "{2}{U}{U}",
                blue_mana: 4,
                kick_once: false,
            },
        );
        assert_eq!(
            session.engine.as_ref().unwrap().state.objects[&33].zone,
            tricerules_core::Zone::Battlefield
        );
        println!("T4: Island and Tekuthal ObjectId=33 played; Ring remains at Night1");

        latest = tekuthal_normal_advance_to_main(
            &mut session,
            root.as_path(),
            &mut rows,
            &mut accepted_commands,
            latest,
            &mut battlefield,
            TEKUTHAL_NORMAL_OPPONENT_ID,
        );
        latest = tekuthal_normal_play_land(
            &mut session,
            root.as_path(),
            &mut rows,
            &mut accepted_commands,
            &latest,
            &mut battlefield,
            TekuthalNormalLandPlan {
                player_id: TEKUTHAL_NORMAL_OPPONENT_ID,
                card_name: "Swamp",
            },
        );
        assert_eq!(
            tekuthal_normal_battlefield_count(&session, TEKUTHAL_NORMAL_OPPONENT_ID, "Swamp"),
            4,
            "a fourth ordinary Swamp avoids a cleanup discard"
        );

        latest = tekuthal_normal_advance_to_main(
            &mut session,
            root.as_path(),
            &mut rows,
            &mut accepted_commands,
            latest,
            &mut battlefield,
            TEKUTHAL_NORMAL_PILOT_ID,
        );
        assert!(
            session
                .engine
                .as_ref()
                .unwrap()
                .state
                .players
                .iter()
                .find(|player| player.id == TEKUTHAL_NORMAL_PILOT_ID)
                .unwrap()
                .hand
                .contains(&49),
            "P17's real T5 draw is ObjectId 49 Island"
        );
        assert_eq!(
            session.engine.as_ref().unwrap().state.objects[&41]
                .counter_count(tricerules_cards::CounterKind::Night),
            2,
            "Ring's ordinary T5 upkeep adds one Night counter"
        );
        latest = tekuthal_normal_play_land(
            &mut session,
            root.as_path(),
            &mut rows,
            &mut accepted_commands,
            &latest,
            &mut battlefield,
            TekuthalNormalLandPlan {
                player_id: TEKUTHAL_NORMAL_PILOT_ID,
                card_name: "Karn's Bastion",
            },
        );
        let final_actions = kaya_batch(&latest)
            .legal_by_player
            .get(&TEKUTHAL_NORMAL_PILOT_ID)
            .expect("current T5 Main1 response includes P17's legal actions");
        assert!(
            final_actions.pending_spell_cast.is_none(),
            "the current T5 Main1 offer has no pending spell payment"
        );
        assert!(
            !final_actions
                .hand_actions
                .iter()
                .any(|offer| offer.kind == HandActionKind::HandActionPlayLand as i32),
            "P17's current T5 Main1 offer has no second land play"
        );
        let state = &session.engine.as_ref().unwrap().state;
        assert_eq!(state.turn_step, tricerules_core::TurnStep::Main1);
        assert_eq!(
            state.players[state.active_player_idx].id,
            TEKUTHAL_NORMAL_PILOT_ID
        );
        assert_eq!(state.priority_player_id(), TEKUTHAL_NORMAL_PILOT_ID);
        assert_eq!(
            tekuthal_normal_battlefield_count(&session, TEKUTHAL_NORMAL_PILOT_ID, "Island"),
            4
        );
        assert_eq!(
            tekuthal_normal_battlefield_count(&session, TEKUTHAL_NORMAL_PILOT_ID, "Karn's Bastion"),
            1
        );
        assert!(!state.objects[&42].tapped, "T5 Bastion is ready");
        assert_eq!(
            state.objects[&36].counter_count(tricerules_cards::CounterKind::Charge),
            1
        );
        assert_eq!(
            state.objects[&41].counter_count(tricerules_cards::CounterKind::Night),
            2
        );
        assert_eq!(state.objects[&33].zone, tricerules_core::Zone::Battlefield);
        assert_eq!(
            state
                .players
                .iter()
                .find(|player| player.id == TEKUTHAL_NORMAL_PILOT_ID)
                .unwrap()
                .life,
            20
        );
        assert!(state.stack.is_empty());
        assert!(state.pending_spell_cast.is_none());
        assert!(state.pending_resolution.is_none());
        assert!(
            state
                .players
                .iter()
                .find(|player| player.id == TEKUTHAL_NORMAL_OPPONENT_ID)
                .unwrap()
                .hand
                .contains(&91),
            "P29's Murder remains in hand"
        );
        latest = tekuthal_normal_activate_bastion(
            &mut session,
            root.as_path(),
            &mut rows,
            &mut accepted_commands,
            latest,
            &mut battlefield,
        );
        let resolved_actions = kaya_batch(&latest)
            .legal_by_player
            .get(&TEKUTHAL_NORMAL_PILOT_ID)
            .expect("current post-Bastion T5 response includes P17's legal actions");
        assert!(resolved_actions.pending_spell_cast.is_none());
        assert!(resolved_actions.pending_ability_activation.is_none());
        let state = &session.engine.as_ref().unwrap().state;
        assert_eq!(state.turn_step, tricerules_core::TurnStep::Main1);
        assert_eq!(
            state.players[state.active_player_idx].id,
            TEKUTHAL_NORMAL_PILOT_ID
        );
        assert_eq!(state.priority_player_id(), TEKUTHAL_NORMAL_PILOT_ID);
        assert!(state.stack.is_empty());
        assert!(state.pending_resolution.is_none());
        assert!(state.pending_ability_activation.is_none());
        assert!(state.objects[&42].tapped, "Bastion paid its tap cost");
        assert!(!state.objects[&41].tapped, "Ring remains untapped");
        assert_eq!(
            state.objects[&41].counter_count(tricerules_cards::CounterKind::Night),
            3
        );
        assert_eq!(
            state.objects[&36].counter_count(tricerules_cards::CounterKind::Charge),
            2
        );
        assert_eq!(
            state
                .players
                .iter()
                .find(|player| player.id == TEKUTHAL_NORMAL_PILOT_ID)
                .unwrap()
                .life,
            20
        );
        assert!(
            state
                .players
                .iter()
                .find(|player| player.id == TEKUTHAL_NORMAL_OPPONENT_ID)
                .unwrap()
                .hand
                .contains(&91),
            "P29's Murder remains available after the Bastion activation"
        );
        let island_card_id = tricerules_cards::registry::global()
            .id_for_name("Island")
            .expect("Island is in the global registry")
            .to_owned();
        let pilot = state
            .players
            .iter()
            .find(|player| player.id == TEKUTHAL_NORMAL_PILOT_ID)
            .unwrap();
        let islands = pilot
            .battlefield
            .iter()
            .filter_map(|object_id| state.objects.get(object_id))
            .filter(|object| object.card_id == island_card_id)
            .collect::<Vec<_>>();
        assert_eq!(islands.len(), 4);
        assert!(islands.iter().all(|island| island.tapped));
        println!(
            "T5 Bastion resolved through both current Proliferate prompts: Islands=4 tapped, Bastion=42 tapped, Chalice Charge=2, Ring Night=3, Ring untapped, P17 life=20, accepted PlayerCommands={accepted_commands}"
        );
        latest = tekuthal_normal_activate_tekuthal(
            &mut session,
            root.as_path(),
            &mut rows,
            &mut accepted_commands,
            latest,
            &mut battlefield,
        );
        let resolved_actions = kaya_batch(&latest)
            .legal_by_player
            .get(&TEKUTHAL_NORMAL_PILOT_ID)
            .expect("current post-Tekuthal T5 response includes P17's legal actions");
        assert!(resolved_actions.pending_spell_cast.is_none());
        assert!(resolved_actions.pending_ability_activation.is_none());
        println!(
            "T5 Tekuthal activation resolved: P17 life=16, Ring Night=2 tapped, Chalice Charge=0, Tekuthal Indestructible=1, accepted PlayerCommands={accepted_commands}"
        );
        latest = tekuthal_normal_cast_murder(
            &mut session,
            root.as_path(),
            &mut rows,
            &mut accepted_commands,
            latest,
            &mut battlefield,
        );
        assert!(latest.ok, "final Murder-resolution response is accepted");
        let final_batch = kaya_batch(&latest);
        let p17_actions = final_batch
            .legal_by_player
            .get(&TEKUTHAL_NORMAL_PILOT_ID)
            .expect("final response publishes the active P17 private offer");
        assert!(p17_actions.pending_spell_cast.is_none());
        assert!(p17_actions.pending_ability_activation.is_none());
        let p29_actions = final_batch
            .legal_by_player
            .get(&TEKUTHAL_NORMAL_OPPONENT_ID)
            .expect("final response retains P29's private action view");
        assert!(p29_actions.pending_spell_cast.is_none());
        assert!(session
            .engine
            .as_ref()
            .expect("normal session has an engine after Murder")
            .state
            .stack
            .is_empty());
        fs::write(
            root.as_path().join("timeline.jsonl"),
            rows.join("\n") + "\n",
        )
        .expect("finish normal-pilot timeline");
        manifest["complete"] = true.into();
        manifest["accepted_commands"] = accepted_commands.into();
        fs::write(root.as_path().join("manifest.json"), manifest.to_string())
            .expect("finish normal-pilot manifest");
        let expected_state = session
            .engine
            .as_ref()
            .expect("normal session has an engine after T5 Main1")
            .diagnostic_snapshot()
            .expect("capture live diagnostic snapshot");
        let capture = load_capture(root.as_path()).expect("load completed normal-pilot capture");
        assert!(
            capture.warnings.is_empty(),
            "capture warnings: {:?}",
            capture.warnings
        );
        let replayed = reconstruct(&capture, &ReplayOptions::default())
            .expect("reconstruct the recorded normal-pilot prefix");
        assert!(
            replayed.warnings.is_empty(),
            "replay warnings: {:?}",
            replayed.warnings
        );
        assert!(
            replayed.differences.is_empty(),
            "replay differences: {:?}",
            replayed.differences
        );
        assert_eq!(
            replayed.accepted_commands,
            u64::try_from(accepted_commands).expect("accepted count fits replay result")
        );
        assert_eq!(replayed.state, expected_state);
        println!(
            "T1-T5 strict normal curve through P29's Murder replayed exactly; accepted PlayerCommands={accepted_commands}, timeline records={}",
            rows.len()
        );
    }
    fn fixture() -> (tempfile::TempDir, Value) {
        fixture_with_decks(vec![])
    }

    fn fixture_with_decks(player_decks: Vec<PlayerDeck>) -> (tempfile::TempDir, Value) {
        fixture_with_decks_and_signet_activation(player_decks, false)
    }

    fn fixture_with_signet_activation(player_decks: Vec<PlayerDeck>) -> (tempfile::TempDir, Value) {
        fixture_with_decks_and_signet_activation(player_decks, true)
    }

    fn fixture_with_decks_and_signet_activation(
        player_decks: Vec<PlayerDeck>,
        include_signet_activation: bool,
    ) -> (tempfile::TempDir, Value) {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("raw")).unwrap();
        let mut session = EngineSession::new(include_signet_activation);
        let start = IpcEnvelope {
            msg: Some(Msg::SessionStart(tricerules_proto::SessionStart {
                game_id: 7,
                seed: u64::MAX,
                player_ids: vec![17, 29],
                player_decks,
                dev_commands_enabled: include_signet_activation,
                diagnostic_capture_enabled: true,
                ..Default::default()
            })),
        };
        let response = session.process(&start).unwrap();
        let manifest = serde_json::json!({"format_version":1,"source":"server","privacy":"server_only",
            "capture_id":"00000000-0000-4000-8000-000000000007", "engine_build":ENGINE_BUILD,
            "card_data_hash":response.card_data_hash,
            "effective_dev_commands_enabled":include_signet_activation,"complete":false});
        fs::write(root.path().join("manifest.json"), manifest.to_string()).unwrap();
        let mut rows = vec![];
        pair(root.path(), &mut rows, &start, &response);
        let actor = session
            .engine
            .as_ref()
            .unwrap()
            .state
            .opening
            .as_ref()
            .unwrap()
            .chooser;
        let command = tricerules_proto::RuledCommand {
            cmd: Some(Cmd::ChooseStartingPlayer(
                tricerules_proto::ChooseStartingPlayer {
                    starting_player_id: if include_signet_activation { 17 } else { 29 },
                },
            )),
        };
        let response =
            record_player_command(&mut session, root.path(), &mut rows, actor, command.clone());
        assert!(
            response.ok,
            "starting-player choice rejected: {}",
            response.error
        );

        if include_signet_activation {
            // Complete both keep decisions before priority becomes available.
            for _ in 0..2 {
                let actor = session
                    .engine
                    .as_ref()
                    .unwrap()
                    .state
                    .opening
                    .as_ref()
                    .unwrap()
                    .mulligan_actor
                    .unwrap();
                let response = record_player_command(
                    &mut session,
                    root.path(),
                    &mut rows,
                    actor,
                    RuledCommand {
                        cmd: Some(Cmd::Mulligan(MulliganDecision { keep: true })),
                    },
                );
                assert!(response.ok, "opening keep rejected: {}", response.error);
            }

            // On the starting player's first turn, two full priority passes advance each step
            // from upkeep through draw to main phase.
            for _ in 0..4 {
                let actor = session.engine.as_ref().unwrap().state.priority_player_id();
                let response = record_player_command(
                    &mut session,
                    root.path(),
                    &mut rows,
                    actor,
                    RuledCommand {
                        cmd: Some(Cmd::PassPriority(PassPriority {})),
                    },
                );
                assert!(response.ok, "priority pass rejected: {}", response.error);
            }
            assert_eq!(
                session.engine.as_ref().unwrap().state.turn_step,
                tricerules_core::TurnStep::Main1
            );

            let response = record_player_command(
                &mut session,
                root.path(),
                &mut rows,
                17,
                RuledCommand {
                    cmd: Some(Cmd::DevCommand(DevCommand {
                        target_player_id: 17,
                        dev: Some(dev_command::Dev::MoveCard(DevMoveCard {
                            card_name: "Arcane Signet".into(),
                            zone: DevZone::Battlefield as i32,
                            ready: true,
                        })),
                    })),
                },
            );
            assert!(response.ok, "Signet dev move rejected: {}", response.error);
            let engine = session.engine.as_ref().unwrap();
            let signet = engine.state.players[0]
                .battlefield
                .iter()
                .copied()
                .find(|object_id| engine.state.objects[object_id].card_id == "arcane_signet")
                .expect("moved Signet is on the battlefield");
            let generation = engine
                .state
                .zone_change_generation
                .get(&signet)
                .copied()
                .unwrap_or(0);
            let stack_len = engine.state.stack.len();
            let response = record_player_command(
                &mut session,
                root.path(),
                &mut rows,
                17,
                RuledCommand {
                    cmd: Some(Cmd::ActivateAbility(ActivateAbility {
                        source_object_id: signet,
                        ability_index: 0,
                        expected_zone_change_generation: generation,
                        mana_option_index: 2,
                        ..Default::default()
                    })),
                },
            );
            assert!(
                response.ok,
                "Signet activation rejected: {}",
                response.error
            );
            let engine = session.engine.as_ref().unwrap();
            assert!(engine.state.objects[&signet].tapped);
            assert_eq!(engine.state.players[0].mana_pool.black, 1);
            assert_eq!(engine.state.stack.len(), stack_len);
        }

        let rejected = IpcEnvelope {
            msg: Some(Msg::PlayerCommand(tricerules_proto::PlayerCommand {
                player_id: 999,
                ruled_command: command.encode_to_vec(),
            })),
        };
        let response = session.process(&rejected).unwrap();
        assert!(!response.ok);
        pair(root.path(), &mut rows, &rejected, &response);
        fs::write(root.path().join("timeline.jsonl"), rows.join("\n") + "\n").unwrap();
        (
            root,
            session
                .engine
                .as_ref()
                .unwrap()
                .diagnostic_snapshot()
                .unwrap(),
        )
    }

    fn record_player_command(
        session: &mut EngineSession,
        directory: &Path,
        rows: &mut Vec<String>,
        player_id: i32,
        command: RuledCommand,
    ) -> IpcResponse {
        let request = IpcEnvelope {
            msg: Some(Msg::PlayerCommand(PlayerCommand {
                player_id,
                ruled_command: command.encode_to_vec(),
            })),
        };
        let response = session.process(&request).unwrap();
        pair(directory, rows, &request, &response);
        response
    }

    fn record_accepted_player_command(
        session: &mut EngineSession,
        directory: &Path,
        rows: &mut Vec<String>,
        player_id: i32,
        command: RuledCommand,
        label: &str,
    ) {
        let response = record_player_command(session, directory, rows, player_id, command);
        assert!(response.ok, "{label} rejected: {}", response.error);
    }

    fn captured_generation(session: &EngineSession, object_id: u32) -> u64 {
        session
            .engine
            .as_ref()
            .expect("started engine session")
            .state
            .zone_change_generation
            .get(&object_id)
            .copied()
            .unwrap_or(0)
    }

    #[test]
    fn resume_plan_preserves_declared_commander_and_no_commander_setups() {
        let decks = vec![
            PlayerDeck {
                player_id: 17,
                mainboard_card_name: vec!["Arcane Signet".into(); 8],
                commander_setup: Some(CommanderSetup {
                    declaration: Some(commander_setup::Declaration::Declared(
                        commander_setup::Declared {
                            card_name: vec!["Atraxa, Praetors' Voice".into()],
                        },
                    )),
                }),
            },
            PlayerDeck {
                player_id: 29,
                mainboard_card_name: vec!["Island".into(); 8],
                commander_setup: Some(CommanderSetup {
                    declaration: Some(commander_setup::Declaration::NoCommander(true)),
                }),
            },
        ];
        let (root, _) = fixture_with_decks(decks.clone());
        let capture = load_capture(root.path()).unwrap();
        let plan = build_resume_plan(&capture, &ReplayOptions::default()).unwrap();
        assert!(reconstruct(&capture, &ReplayOptions::default())
            .unwrap()
            .differences
            .is_empty());
        assert_eq!(plan.display_decks, decks);
        assert_eq!(plan.session_start.unwrap().player_decks, decks);
    }

    #[test]
    fn replay_repeats_commander_signet_choice_and_mana_pool() {
        let decks = vec![
            PlayerDeck {
                player_id: 17,
                mainboard_card_name: vec!["Arcane Signet".into(); 8],
                commander_setup: Some(CommanderSetup {
                    declaration: Some(commander_setup::Declaration::Declared(
                        commander_setup::Declared {
                            card_name: vec!["Atraxa, Praetors' Voice".into()],
                        },
                    )),
                }),
            },
            PlayerDeck {
                player_id: 29,
                mainboard_card_name: vec!["Island".into(); 8],
                commander_setup: Some(CommanderSetup {
                    declaration: Some(commander_setup::Declaration::NoCommander(true)),
                }),
            },
        ];
        let (root, expected) = fixture_with_signet_activation(decks.clone());
        let capture = load_capture(root.path()).unwrap();
        let replayed = reconstruct(&capture, &ReplayOptions::default()).unwrap();
        assert!(replayed.differences.is_empty());
        assert_eq!(replayed.accepted_commands, 9);
        assert_eq!(replayed.state, expected);
        assert_eq!(
            replayed.state["state"]["players"][0]["mana_pool"]["black"],
            1
        );

        let plan = build_resume_plan(&capture, &ReplayOptions::default()).unwrap();
        assert_eq!(plan.stop_after, 9);
        assert!(plan.effective_dev_commands_enabled);
        assert_eq!(plan.display_decks, decks);
        assert_eq!(plan.session_start.unwrap().player_decks, decks);
    }

    #[test]
    fn aetheric_activated_modes_replay_from_explicit_id_session_capture() {
        const CONTROLLER: i32 = 17;
        const OPPONENT: i32 = 29;

        let decks = [CONTROLLER, OPPONENT]
            .into_iter()
            .map(|player_id| PlayerDeck {
                player_id,
                mainboard_card_name: vec!["Forest".into(); 8],
                commander_setup: Some(CommanderSetup {
                    declaration: Some(commander_setup::Declaration::NoCommander(true)),
                }),
            })
            .collect();
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("raw")).unwrap();
        let mut session = EngineSession::new(true);
        let start = IpcEnvelope {
            msg: Some(Msg::SessionStart(tricerules_proto::SessionStart {
                game_id: 7,
                seed: 26_102_813,
                player_ids: vec![CONTROLLER, OPPONENT],
                player_decks: decks,
                dev_commands_enabled: true,
                diagnostic_capture_enabled: true,
                ..Default::default()
            })),
        };
        let start_response = session.process(&start).expect("session start response");
        assert!(
            start_response.ok,
            "session start rejected: {}",
            start_response.error
        );
        let manifest = serde_json::json!({
            "format_version": 1,
            "source": "server",
            "privacy": "server_only",
            "capture_id": "00000000-0000-4000-8000-000000000008",
            "engine_build": ENGINE_BUILD,
            "card_data_hash": start_response.card_data_hash.clone(),
            "effective_dev_commands_enabled": true,
            "complete": false
        });
        fs::write(root.path().join("manifest.json"), manifest.to_string()).unwrap();
        let mut rows = Vec::new();
        pair(root.path(), &mut rows, &start, &start_response);

        let chooser = session
            .engine
            .as_ref()
            .expect("started engine")
            .state
            .opening
            .as_ref()
            .expect("opening state")
            .chooser;
        record_accepted_player_command(
            &mut session,
            root.path(),
            &mut rows,
            chooser,
            RuledCommand {
                cmd: Some(Cmd::ChooseStartingPlayer(
                    tricerules_proto::ChooseStartingPlayer {
                        starting_player_id: CONTROLLER,
                    },
                )),
            },
            "choose starting player",
        );
        for _ in 0..2 {
            let actor = session
                .engine
                .as_ref()
                .expect("started engine")
                .state
                .opening
                .as_ref()
                .expect("opening state")
                .mulligan_actor
                .expect("mulligan actor");
            record_accepted_player_command(
                &mut session,
                root.path(),
                &mut rows,
                actor,
                RuledCommand {
                    cmd: Some(Cmd::Mulligan(MulliganDecision { keep: true })),
                },
                "keep opening hand",
            );
        }
        for _ in 0..4 {
            let actor = session
                .engine
                .as_ref()
                .expect("started engine")
                .state
                .priority_player_id();
            record_accepted_player_command(
                &mut session,
                root.path(),
                &mut rows,
                actor,
                RuledCommand {
                    cmd: Some(Cmd::PassPriority(PassPriority {})),
                },
                "advance to main phase",
            );
        }
        assert_eq!(
            session.engine.as_ref().unwrap().state.turn_step,
            tricerules_core::TurnStep::Main1
        );

        for card_name in ["Aetheric Amplifier", "Aetheric Amplifier", "Mind Stone"] {
            let actor = session
                .engine
                .as_ref()
                .expect("started engine")
                .state
                .priority_player_id();
            record_accepted_player_command(
                &mut session,
                root.path(),
                &mut rows,
                actor,
                RuledCommand {
                    cmd: Some(Cmd::DevCommand(DevCommand {
                        target_player_id: CONTROLLER,
                        dev: Some(dev_command::Dev::PutCardInZone(DevPutCardInZone {
                            card_name: card_name.into(),
                            zone: DevZone::Battlefield as i32,
                            ready: true,
                        })),
                    })),
                },
                "build logged Aetheric board state",
            );
        }
        let actor = session
            .engine
            .as_ref()
            .expect("started engine")
            .state
            .priority_player_id();
        record_accepted_player_command(
            &mut session,
            root.path(),
            &mut rows,
            actor,
            RuledCommand {
                cmd: Some(Cmd::DevCommand(DevCommand {
                    target_player_id: CONTROLLER,
                    dev: Some(dev_command::Dev::AddMana(DevAddMana {
                        c: 8,
                        ..Default::default()
                    })),
                })),
            },
            "add logged activation mana",
        );

        let (first_source, second_source, target) = {
            let engine = session.engine.as_ref().unwrap();
            let sources: Vec<_> = engine.state.players[0]
                .battlefield
                .iter()
                .copied()
                .filter(|object_id| engine.state.objects[object_id].card_id == "aetheric_amplifier")
                .collect();
            assert_eq!(sources.len(), 2);
            let target = engine.state.players[0]
                .battlefield
                .iter()
                .copied()
                .find(|object_id| engine.state.objects[object_id].card_id == "mind_stone")
                .expect("logged Mind Stone target");
            (sources[0], sources[1], target)
        };
        assert_ne!(first_source, second_source);

        let first_source_generation = captured_generation(&session, first_source);
        let target_generation = captured_generation(&session, target);
        let first_revision = session.engine.as_ref().unwrap().state.command_index;
        let first_activation = RuledCommand {
            cmd: Some(Cmd::ActivateAbility(ActivateAbility {
                source_object_id: first_source,
                ability_index: 1,
                expected_zone_change_generation: first_source_generation,
                selected_modes: vec![SelectedSpellMode {
                    mode_index: 0,
                    targets: vec![TargetRef {
                        object_id: target,
                        group_index: 0,
                        kind: TargetRefKind::Permanent as i32,
                        expected_zone_change_generation: Some(target_generation),
                        ..Default::default()
                    }],
                }],
                payment: Some(PaymentSelection {
                    expected_state_revision: first_revision,
                    source: Some(CostObjectRef {
                        object_id: first_source,
                        zone_change_generation: first_source_generation,
                    }),
                    mana: Some(PaymentMana {
                        c: 4,
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            })),
        };
        let first_activation_bytes = first_activation.encode_to_vec();
        let actor = session.engine.as_ref().unwrap().state.priority_player_id();
        record_accepted_player_command(
            &mut session,
            root.path(),
            &mut rows,
            actor,
            first_activation,
            "activate the targeted mode",
        );
        {
            let engine = session.engine.as_ref().unwrap();
            let item = engine.state.stack.last().expect("ability on stack");
            assert!(item.targets.is_empty(), "modal targets stay on their mode");
            assert_eq!(item.chosen_modes.len(), 1);
            assert_eq!(
                item.chosen_modes[0].mode_id.as_str(),
                "double_any_permanent"
            );
            assert_eq!(item.chosen_modes[0].targets.len(), 1);
            assert_eq!(item.chosen_modes[0].targets[0].object_id, target);
            assert_eq!(
                item.chosen_modes[0].targets[0].zone_change_generation,
                Some(target_generation)
            );
        }
        for _ in 0..2 {
            let actor = session.engine.as_ref().unwrap().state.priority_player_id();
            record_accepted_player_command(
                &mut session,
                root.path(),
                &mut rows,
                actor,
                RuledCommand {
                    cmd: Some(Cmd::PassPriority(PassPriority {})),
                },
                "resolve the targeted mode",
            );
        }
        assert!(session.engine.as_ref().unwrap().state.stack.is_empty());
        assert!(session.engine.as_ref().unwrap().state.objects[&target]
            .counters
            .is_empty());

        let second_source_generation = captured_generation(&session, second_source);
        let second_revision = session.engine.as_ref().unwrap().state.command_index;
        let second_activation = RuledCommand {
            cmd: Some(Cmd::ActivateAbility(ActivateAbility {
                source_object_id: second_source,
                ability_index: 1,
                expected_zone_change_generation: second_source_generation,
                selected_modes: vec![SelectedSpellMode {
                    mode_index: 1,
                    targets: Vec::new(),
                }],
                payment: Some(PaymentSelection {
                    expected_state_revision: second_revision,
                    source: Some(CostObjectRef {
                        object_id: second_source,
                        zone_change_generation: second_source_generation,
                    }),
                    mana: Some(PaymentMana {
                        c: 4,
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
                ..Default::default()
            })),
        };
        let second_activation_bytes = second_activation.encode_to_vec();
        let actor = session.engine.as_ref().unwrap().state.priority_player_id();
        record_accepted_player_command(
            &mut session,
            root.path(),
            &mut rows,
            actor,
            second_activation,
            "activate the nontargeted mode",
        );
        {
            let engine = session.engine.as_ref().unwrap();
            let item = engine.state.stack.last().expect("ability on stack");
            assert!(item.targets.is_empty());
            assert_eq!(item.chosen_modes.len(), 1);
            assert_eq!(
                item.chosen_modes[0].mode_id.as_str(),
                "double_your_counters"
            );
            assert!(item.chosen_modes[0].targets.is_empty());
            assert!(engine.state.players[0].counters.is_empty());
        }
        for _ in 0..2 {
            let actor = session.engine.as_ref().unwrap().state.priority_player_id();
            record_accepted_player_command(
                &mut session,
                root.path(),
                &mut rows,
                actor,
                RuledCommand {
                    cmd: Some(Cmd::PassPriority(PassPriority {})),
                },
                "resolve the nontargeted mode",
            );
        }
        let live = session.engine.as_ref().unwrap();
        assert!(live.state.stack.is_empty());
        assert!(live.state.objects[&target].counters.is_empty());
        assert!(live.state.players[0].counters.is_empty());
        assert_eq!(live.state.players[0].mana_pool.colorless, 0);
        let expected_state = live.diagnostic_snapshot().unwrap();

        fs::write(root.path().join("timeline.jsonl"), rows.join("\n") + "\n").unwrap();
        let capture = load_capture(root.path()).unwrap();
        assert!(capture.warnings.is_empty());

        let mut activation_requests = Vec::new();
        for exchange in &capture.exchanges {
            let Some(Msg::PlayerCommand(player_command)) = &exchange.request.msg else {
                continue;
            };
            let command = RuledCommand::decode(player_command.ruled_command.as_slice())
                .expect("decode captured activation command");
            if matches!(&command.cmd, Some(Cmd::ActivateAbility(_))) {
                activation_requests.push((
                    exchange.sequence,
                    player_command.player_id,
                    player_command.ruled_command.clone(),
                    command,
                    exchange,
                ));
            }
        }
        assert_eq!(activation_requests.len(), 2);
        for (index, (sequence, player_id, raw_command, command, exchange)) in
            activation_requests.iter().enumerate()
        {
            let expected_bytes = if index == 0 {
                &first_activation_bytes
            } else {
                &second_activation_bytes
            };
            assert_eq!(*player_id, CONTROLLER);
            assert_eq!(
                raw_command, expected_bytes,
                "the command bytes stay unchanged"
            );

            let raw_request = Record::decode(
                fs::read(capture.directory.join(format!("raw/{sequence:012}.pb")))
                    .expect("read captured raw request record")
                    .as_slice(),
            )
            .expect("decode captured raw request record");
            assert_eq!(raw_request.kind, "engine_request");
            assert_eq!(raw_request.message_type, "ruled.v1.IpcEnvelope");
            assert_eq!(raw_request.payload, exchange.request.encode_to_vec());

            let response = exchange.response.as_ref().expect("captured response");
            let raw_response = Record::decode(
                fs::read(
                    capture
                        .directory
                        .join(format!("raw/{:012}.pb", sequence + 1)),
                )
                .expect("read captured raw response record")
                .as_slice(),
            )
            .expect("decode captured raw response record");
            assert_eq!(raw_response.kind, "engine_response");
            assert_eq!(raw_response.message_type, "ruled.v1.IpcResponse");
            assert_eq!(raw_response.correlation_id, raw_request.correlation_id);
            assert_eq!(raw_response.payload, response.encode_to_vec());

            let Some(Cmd::ActivateAbility(activation)) = &command.cmd else {
                unreachable!("filtered activation request")
            };
            assert!(
                activation.targets.is_empty(),
                "no flattened target duplicate"
            );
            assert_eq!(
                activation.expected_zone_change_generation,
                if index == 0 {
                    first_source_generation
                } else {
                    second_source_generation
                }
            );
            assert_eq!(activation.selected_modes.len(), 1);
            let selected_mode = &activation.selected_modes[0];
            assert_eq!(selected_mode.mode_index, if index == 0 { 0 } else { 1 });
            let payment = activation.payment.as_ref().expect("explicit payment");
            assert_eq!(
                payment.expected_state_revision,
                if index == 0 {
                    first_revision
                } else {
                    second_revision
                }
            );
            let source = payment
                .source
                .as_ref()
                .expect("generation-bound payment source");
            assert_eq!(
                source.object_id,
                if index == 0 {
                    first_source
                } else {
                    second_source
                }
            );
            assert_eq!(
                source.zone_change_generation,
                if index == 0 {
                    first_source_generation
                } else {
                    second_source_generation
                }
            );
            if index == 0 {
                assert_eq!(selected_mode.targets.len(), 1);
                assert_eq!(selected_mode.targets[0].object_id, target);
                assert_eq!(
                    selected_mode.targets[0].expected_zone_change_generation,
                    Some(target_generation)
                );
            } else {
                assert!(selected_mode.targets.is_empty());
            }
        }

        let replayed = reconstruct(&capture, &ReplayOptions::default()).unwrap();
        assert!(
            replayed.differences.is_empty(),
            "{:?}",
            replayed.differences
        );
        assert_eq!(replayed.accepted_commands, 17);
        assert_eq!(replayed.state, expected_state);
    }

    fn pair(root: &Path, rows: &mut Vec<String>, request: &IpcEnvelope, response: &IpcResponse) {
        let correlation = format!("engine-{}", rows.len() + 1);
        for (kind, name, payload) in [
            (
                "engine_request",
                "ruled.v1.IpcEnvelope",
                request.encode_to_vec(),
            ),
            (
                "engine_response",
                "ruled.v1.IpcResponse",
                response.encode_to_vec(),
            ),
        ] {
            let sequence = rows.len() as u64 + 1;
            let record = Record {
                format_version: 1,
                sequence,
                kind: kind.into(),
                message_type: name.into(),
                payload,
                correlation_id: correlation.clone(),
            };
            let raw = format!("raw/{sequence:012}.pb");
            fs::write(root.join(&raw), record.encode_to_vec()).unwrap();
            rows.push(
                serde_json::json!({"format_version":1,"sequence":sequence.to_string(),"kind":kind,
                "message_type":name,"correlation_id":correlation,"raw_ref":raw})
                .to_string(),
            );
        }
    }

    #[test]
    fn resume_plan_preserves_players_seed_accepted_prefix_and_state_checks() {
        let (root, _) = fixture();
        let capture = load_capture(root.path()).unwrap();
        let plan = build_resume_plan(&capture, &ReplayOptions::default()).unwrap();
        assert_eq!(
            plan.session_start.as_ref().unwrap().player_ids,
            vec![17, 29]
        );
        assert_eq!(plan.session_start.as_ref().unwrap().seed, u64::MAX);
        assert_eq!(plan.steps.len(), 1);
        assert_eq!(plan.stop_after, 1);
        assert_eq!(plan.display_decks.len(), 2);
        assert!(!plan.display_decks[0].mainboard_card_name.is_empty());
        assert_eq!(plan.initial_state_sha256.len(), 32);
        assert_eq!(plan.steps[0].expected_state_sha256.len(), 32);
        let initial = build_resume_plan(
            &capture,
            &ReplayOptions {
                stop_after: Some(0),
                ..Default::default()
            },
        )
        .unwrap();
        assert!(initial.steps.is_empty());
    }

    #[test]
    fn reconstructs_nonconsecutive_seats_and_skips_rejected_gameplay() {
        let (root, expected) = fixture();
        let capture = load_capture(root.path()).unwrap();
        let result = reconstruct(&capture, &ReplayOptions::default()).unwrap();
        assert_eq!(result.accepted_commands, 1);
        assert_eq!(result.state, expected);
        assert!(result.differences.is_empty());
        let initial = reconstruct(
            &capture,
            &ReplayOptions {
                stop_after: Some(0),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(initial.accepted_commands, 0);
        assert_ne!(initial.state, expected);
        let retry = reconstruct(
            &capture,
            &ReplayOptions {
                retry_sequence: Some(5),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(retry.accepted_commands, 1);
        assert_eq!(retry.retry.unwrap()["ok"], false);
    }

    #[test]
    fn refuses_incompatible_build_and_reports_changed_results() {
        let (root, _) = fixture();
        let mut capture = load_capture(root.path()).unwrap();
        capture.manifest["engine_build"] = "old-build".into();
        assert!(reconstruct(&capture, &ReplayOptions::default())
            .unwrap_err()
            .contains("build"));
        capture.exchanges[1]
            .response
            .as_mut()
            .unwrap()
            .batch
            .as_mut()
            .unwrap()
            .events
            .clear();
        let result = reconstruct(
            &capture,
            &ReplayOptions {
                allow_build_mismatch: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(!result.differences.is_empty());
    }

    #[test]
    fn refuses_path_traversal_and_marks_truncated_tail() {
        let (root, _) = fixture();
        let path = root.path().join("timeline.jsonl");
        let text = fs::read_to_string(&path).unwrap();
        fs::write(&path, format!("{text}{{\"sequence\":")).unwrap();
        let capture = load_capture(root.path()).unwrap();
        assert!(!capture.warnings.is_empty());
        fs::write(&path, text.replace("raw/000000000001.pb", "../secret.pb")).unwrap();
        assert!(load_capture(root.path()).unwrap_err().contains("path"));
    }

    fn kaya_player_deck(player_id: i32, pilot: bool) -> PlayerDeck {
        let mut mainboard_card_name = if pilot {
            vec!["Kaya, Intangible Slayer".to_string(); 4]
        } else {
            Vec::new()
        };
        if pilot {
            mainboard_card_name.extend(vec!["Plains".to_string(); 28]);
            mainboard_card_name.extend(vec!["Swamp".to_string(); 28]);
        } else {
            mainboard_card_name.extend(vec!["Plains".to_string(); 60]);
        }
        PlayerDeck {
            player_id,
            mainboard_card_name,
            commander_setup: Some(CommanderSetup {
                declaration: Some(commander_setup::Declaration::NoCommander(true)),
            }),
        }
    }

    fn kaya_player_command(
        session: &mut EngineSession,
        player_id: i32,
        command: RuledCommand,
    ) -> IpcResponse {
        let response = session
            .process(&IpcEnvelope {
                msg: Some(Msg::PlayerCommand(PlayerCommand {
                    player_id,
                    ruled_command: command.encode_to_vec(),
                })),
            })
            .expect("Kaya player command keeps the session open");
        assert!(
            response.ok,
            "player {player_id} command rejected: {}",
            response.error
        );
        assert!(
            response.batch.is_some(),
            "accepted command returns its current batch"
        );
        response
    }

    fn kaya_pass_priority(session: &mut EngineSession, mut latest: IpcResponse) -> IpcResponse {
        let mut player_id = session
            .engine
            .as_ref()
            .expect("Kaya session has an engine")
            .state
            .priority_player_id();
        let cleanup_discard_count = session
            .engine
            .as_ref()
            .and_then(|engine| {
                let state = &engine.state;
                let player = state.players.iter().find(|player| player.id == player_id)?;
                Some(
                    if state.turn_step == tricerules_core::TurnStep::Cleanup
                        && !state.cleanup_priority_active
                        && state.cleanup_discard_player == Some(player_id)
                    {
                        player.hand.len().saturating_sub(7)
                    } else {
                        0
                    },
                )
            })
            .expect("Kaya priority player is in the session cleanup state");
        // Both test decks contain only basics and Kaya, so the default maximum of seven applies.
        let discard_count = cleanup_discard_count;
        if discard_count > 0 {
            let legal_actions = kaya_batch(&latest)
                .legal_by_player
                .get(&player_id)
                .expect("the cleanup discard player receives current legal actions");
            let cleanup_offers = legal_actions
                .hand_actions
                .iter()
                .filter(|offer| {
                    offer.kind == HandActionKind::HandActionCleanupDiscard as i32
                        && offer.card_name != "Kaya, Intangible Slayer"
                })
                .take(discard_count)
                .map(|offer| offer.hand_index)
                .collect::<Vec<_>>();
            assert_eq!(
                cleanup_offers.len(),
                discard_count,
                "cleanup offers include enough non-Kaya cards to discard"
            );
            latest = kaya_player_command(
                session,
                player_id,
                RuledCommand {
                    cmd: Some(Cmd::DiscardToHandSize(DiscardToHandSize {
                        hand_card_indices: cleanup_offers,
                    })),
                },
            );
            player_id = session
                .engine
                .as_ref()
                .expect("Kaya session has an engine")
                .state
                .priority_player_id();
            assert!(
                kaya_batch(&latest).legal_by_player.contains_key(&player_id),
                "the post-discard response publishes current actions for the new priority player"
            );
        }
        kaya_player_command(
            session,
            player_id,
            RuledCommand {
                cmd: Some(Cmd::PassPriority(PassPriority {})),
            },
        )
    }

    fn kaya_batch(response: &IpcResponse) -> &tricerules_proto::ruled::v1::RuledEventBatch {
        response
            .batch
            .as_ref()
            .expect("Kaya session response has a current event batch")
    }

    fn kaya_starting_player_offer(
        response: &IpcResponse,
    ) -> Option<tricerules_proto::ruled::v1::OpeningActions> {
        kaya_batch(response)
            .legal_by_player
            .values()
            .filter_map(|actions| actions.opening.as_ref())
            .find(|opening| !opening.eligible_starting_player_ids.is_empty())
            .cloned()
    }

    fn kaya_mulligan_offer(
        response: &IpcResponse,
    ) -> Option<tricerules_proto::ruled::v1::OpeningActions> {
        kaya_batch(response)
            .legal_by_player
            .values()
            .filter_map(|actions| actions.opening.as_ref())
            .find(|opening| opening.can_keep)
            .cloned()
    }

    fn kaya_capture_battlefield(
        response: &IpcResponse,
        player_id: i32,
        battlefield: &mut Vec<BattlefieldObject>,
    ) {
        for event in &kaya_batch(response).events {
            let Some(Ev::ZoneView(view)) = event.ev.as_ref() else {
                continue;
            };
            if view.battlefields_unchanged {
                continue;
            }
            if let Some(player_view) = view
                .per_player
                .iter()
                .find(|player_view| player_view.player_id == player_id)
            {
                *battlefield = player_view.battlefield_objects.clone();
            }
        }
    }

    fn kaya_available_mana_abilities(
        battlefield: &[BattlefieldObject],
    ) -> Vec<(u32, u64, u32, String)> {
        battlefield
            .iter()
            .filter(|permanent| !permanent.tapped)
            .flat_map(|permanent| {
                permanent
                    .activated_abilities
                    .iter()
                    .filter(|ability| ability.is_mana_ability && ability.activatable)
                    .map(|ability| {
                        (
                            permanent.object_id,
                            permanent.zone_change_generation,
                            ability.ability_index,
                            ability.mana_produced.clone(),
                        )
                    })
            })
            .collect()
    }

    fn kaya_output_has_color(output: &str, color: char) -> bool {
        output.to_ascii_uppercase().contains(color)
    }

    fn kaya_can_pay_from_offered_lands(battlefield: &[BattlefieldObject]) -> bool {
        let mana = kaya_available_mana_abilities(battlefield);
        let white = mana
            .iter()
            .filter(|ability| kaya_output_has_color(&ability.3, 'W'))
            .count();
        let black = mana
            .iter()
            .filter(|ability| kaya_output_has_color(&ability.3, 'B'))
            .count();
        mana.len() >= 7 && white >= 2 && black >= 2
    }

    fn kaya_on_battlefield(session: &EngineSession) -> Option<(u32, u64)> {
        let engine = session.engine.as_ref()?;
        let player = engine.state.player_idx(17)?;
        engine.state.players[player]
            .battlefield
            .iter()
            .find_map(|object_id| {
                let object = engine.state.objects.get(object_id)?;
                if object.card_id == "kaya,_intangible_slayer" {
                    engine
                        .state
                        .zone_change_generation
                        .get(object_id)
                        .map(|generation| (*object_id, *generation))
                } else {
                    None
                }
            })
    }

    fn kaya_choice(
        response: &IpcResponse,
        choice_kind: ChoiceKind,
    ) -> Option<tricerules_proto::ruled::v1::ResolutionChoiceRequired> {
        kaya_batch(response)
            .events
            .iter()
            .find_map(|event| match event.ev.as_ref() {
                Some(Ev::ResolutionChoiceRequired(choice))
                    if choice.choice_kind == choice_kind as i32 =>
                {
                    Some(choice.clone())
                }
                _ => None,
            })
    }

    fn ripples_pilot_pass_priority(
        session: &mut EngineSession,
        mut latest: IpcResponse,
    ) -> IpcResponse {
        const CHASM_NAME: &str = "Chasm Skulker";
        const RIPPLES_NAME: &str = "Ripples of Potential";

        let mut player_id = session
            .engine
            .as_ref()
            .expect("Ripples pilot session has an engine")
            .state
            .priority_player_id();
        let discard_count = {
            let state = &session.engine.as_ref().unwrap().state;
            if state.turn_step == tricerules_core::TurnStep::Cleanup
                && !state.cleanup_priority_active
                && state.cleanup_discard_player == Some(player_id)
            {
                state
                    .players
                    .iter()
                    .find(|player| player.id == player_id)
                    .expect("Ripples cleanup actor is in the game")
                    .hand
                    .len()
                    .saturating_sub(7)
            } else {
                0
            }
        };
        if discard_count > 0 {
            let legal_actions = kaya_batch(&latest)
                .legal_by_player
                .get(&player_id)
                .expect("current cleanup actor receives hand offers");
            let cleanup_offers = legal_actions
                .hand_actions
                .iter()
                .filter(|offer| {
                    offer.kind == HandActionKind::HandActionCleanupDiscard as i32
                        && (player_id != 17
                            || ![CHASM_NAME, RIPPLES_NAME].contains(&offer.card_name.as_str()))
                })
                .take(discard_count)
                .map(|offer| offer.hand_index)
                .collect::<Vec<_>>();
            assert_eq!(
                cleanup_offers.len(),
                discard_count,
                "current cleanup offers include enough cards while preserving pilot spells"
            );
            latest = kaya_player_command(
                session,
                player_id,
                RuledCommand {
                    cmd: Some(Cmd::DiscardToHandSize(DiscardToHandSize {
                        hand_card_indices: cleanup_offers,
                    })),
                },
            );
            player_id = session.engine.as_ref().unwrap().state.priority_player_id();
            assert!(
                kaya_batch(&latest).legal_by_player.contains_key(&player_id),
                "post-discard response publishes current priority actions"
            );
        }
        kaya_player_command(
            session,
            player_id,
            RuledCommand {
                cmd: Some(Cmd::PassPriority(PassPriority {})),
            },
        )
    }

    fn ripples_pilot_cast_from_current_islands(
        session: &mut EngineSession,
        mut latest: IpcResponse,
        battlefield: &mut Vec<BattlefieldObject>,
        player_id: i32,
        card_name: &str,
        mana_count: u32,
    ) -> IpcResponse {
        let spell = kaya_batch(&latest)
            .legal_by_player
            .get(&player_id)
            .expect("active pilot receives current legal actions")
            .hand_actions
            .iter()
            .find(|offer| {
                offer.kind == HandActionKind::HandActionCastSpell as i32
                    && offer.card_name == card_name
            })
            .cloned()
            .expect("spell is in the current cast-spell hand offers");
        let mana_sources = kaya_available_mana_abilities(battlefield)
            .into_iter()
            .filter(|source| kaya_output_has_color(&source.3, 'U'))
            .collect::<Vec<_>>();
        assert!(
            mana_sources.len() >= mana_count as usize,
            "current Islands provide enough blue mana for {card_name}"
        );

        latest = kaya_player_command(
            session,
            player_id,
            RuledCommand {
                cmd: Some(Cmd::BeginSpellCast(BeginSpellCast {
                    announcement: Some(tricerules_proto::ruled::v1::SpellCastAnnouncement {
                        source: Some(CastSource {
                            location: Some(
                                tricerules_proto::ruled::v1::cast_source::Location::HandIndex(
                                    spell.hand_index,
                                ),
                            ),
                            expected_zone_change_generation: None,
                        }),
                        face_index: spell.face_index,
                        cast_method: CastMethod::Normal as i32,
                        ..Default::default()
                    }),
                })),
            },
        );

        let mut produced = PaymentMana::default();
        for (object_id, generation, ability_index, output) in
            mana_sources.into_iter().take(mana_count as usize)
        {
            let current_source = kaya_available_mana_abilities(battlefield)
                .into_iter()
                .find(|source| {
                    source.0 == object_id && source.1 == generation && source.2 == ability_index
                })
                .expect("latest battlefield view retains the current Island mana offer");
            assert_eq!(current_source.3, output);
            assert!(kaya_output_has_color(&output, 'U'));
            produced.u += 1;
            latest = kaya_player_command(
                session,
                player_id,
                RuledCommand {
                    cmd: Some(Cmd::ActivateAbility(ActivateAbility {
                        source_object_id: object_id,
                        ability_index,
                        source_zone: AbilitySourceZone::Battlefield as i32,
                        expected_zone_change_generation: generation,
                        ..Default::default()
                    })),
                },
            );
            kaya_capture_battlefield(&latest, player_id, battlefield);
        }
        assert_eq!(produced.u, mana_count);

        let pending = kaya_batch(&latest)
            .legal_by_player
            .get(&player_id)
            .and_then(|actions| actions.pending_spell_cast.as_ref())
            .expect("current spell payment offer survives Island activations");
        let payment = pending
            .payment_preview
            .as_ref()
            .expect("pending spell cast publishes its current payment preview");
        let transaction_id = pending.transaction_id;
        let revision = payment.revision;
        let payment_query = session
            .process(&IpcEnvelope {
                msg: Some(Msg::PaymentQuery(PaymentQuery {
                    player_id,
                    preview: Some(PreviewPayment {
                        transaction_id,
                        revision,
                        commit_spell_cast: Some(CommitSpellCast {
                            transaction_id,
                            payment: Some(PaymentSelection {
                                expected_state_revision: revision,
                                mana: Some(produced),
                                ..Default::default()
                            }),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }),
                })),
            })
            .expect("PaymentQuery keeps the Ripples pilot session open");
        assert!(
            payment_query.ok,
            "{card_name} payment preview: {}",
            payment_query.error
        );
        let preview = payment_query
            .batch
            .and_then(|batch| batch.payment_preview)
            .expect("PaymentQuery returns the current spell payment preview");
        assert!(
            preview.valid && preview.complete,
            "{card_name} payment: {preview:?}"
        );
        latest = kaya_player_command(
            session,
            player_id,
            RuledCommand {
                cmd: Some(Cmd::CommitSpellCast(CommitSpellCast {
                    transaction_id,
                    payment: preview.selection,
                    restricted_mana: preview.restricted_mana,
                })),
            },
        );
        kaya_capture_battlefield(&latest, player_id, battlefield);
        latest
    }

    #[test]
    fn jace_normal_session_reaches_triggered_private_discard() {
        const JACE_NAME: &str = "Jace, Cunning Castaway";
        const JACE_CARD_ID: &str = "jace,_cunning_castaway";

        let make_deck = |player_id: i32, jace_count: usize| {
            let mut mainboard_card_name = vec![JACE_NAME.to_string(); jace_count];
            mainboard_card_name.extend(vec!["Island".to_string(); 60 - jace_count]);
            PlayerDeck {
                player_id,
                mainboard_card_name,
                commander_setup: Some(CommanderSetup {
                    declaration: Some(commander_setup::Declaration::NoCommander(true)),
                }),
            }
        };
        let player_decks = vec![make_deck(17, 4), make_deck(29, 0)];
        assert_eq!(player_decks[0].mainboard_card_name.len(), 60);
        assert_eq!(player_decks[1].mainboard_card_name.len(), 60);

        let mut session = EngineSession::new(false);
        let validation = session
            .process(&IpcEnvelope {
                msg: Some(Msg::ValidateDeck(ValidateDeck {
                    card_names: vec![JACE_NAME.into(), "Island".into()],
                })),
            })
            .expect("deck validation leaves the session open");
        assert!(
            validation.ok,
            "complete Jace deck validates: {}",
            validation.error
        );
        assert!(validation.missing_card_names.is_empty());

        let mut latest = session
            .process(&IpcEnvelope {
                msg: Some(Msg::SessionStart(SessionStart {
                    game_id: 7_281_029,
                    seed: 20_261_029,
                    player_ids: vec![17, 29],
                    player_decks,
                    dev_commands_enabled: false,
                    diagnostic_capture_enabled: false,
                    ..Default::default()
                })),
            })
            .expect("SessionStart keeps the normal game session open");
        assert!(latest.ok, "normal Jace GameStart: {}", latest.error);
        assert!(!latest.effective_dev_commands_enabled);

        let starting = kaya_starting_player_offer(&latest)
            .expect("GameStart publishes the eligible starting-player offer");
        assert!(starting.eligible_starting_player_ids.contains(&17));
        latest = kaya_player_command(
            &mut session,
            starting.deciding_player_id,
            RuledCommand {
                cmd: Some(Cmd::ChooseStartingPlayer(ChooseStartingPlayer {
                    starting_player_id: 17,
                })),
            },
        );
        while let Some(opening) = kaya_mulligan_offer(&latest) {
            latest = kaya_player_command(
                &mut session,
                opening.deciding_player_id,
                RuledCommand {
                    cmd: Some(Cmd::Mulligan(MulliganDecision { keep: true })),
                },
            );
        }

        for _ in 0..4 {
            latest = kaya_pass_priority(&mut session, latest);
        }
        {
            let state = &session.engine.as_ref().unwrap().state;
            assert_eq!(state.turn_step, tricerules_core::TurnStep::Main1);
            assert_eq!(state.players[state.active_player_idx].id, 17);
        }

        let mut battlefield = Vec::new();
        kaya_capture_battlefield(&latest, 17, &mut battlefield);
        let jace_from_view = |objects: &[BattlefieldObject]| {
            objects
                .iter()
                .find(|object| object.card_id == JACE_CARD_ID)
                .cloned()
        };

        let mut jace_resolved = false;
        for _ in 0..512 {
            let (active_player_id, priority_player_id, main_phase) = {
                let state = &session.engine.as_ref().unwrap().state;
                (
                    state.players[state.active_player_idx].id,
                    state.priority_player_id(),
                    state.turn_step == tricerules_core::TurnStep::Main1,
                )
            };

            if active_player_id == 17 && priority_player_id == 17 && main_phase {
                let actions = kaya_batch(&latest)
                    .legal_by_player
                    .get(&17)
                    .expect("active player receives current legal actions")
                    .clone();
                if let Some(land) = actions
                    .hand_actions
                    .iter()
                    .find(|action| action.kind == HandActionKind::HandActionPlayLand as i32)
                    .cloned()
                {
                    latest = kaya_player_command(
                        &mut session,
                        17,
                        RuledCommand {
                            cmd: Some(Cmd::PlayLand(PlayLand {
                                source: Some(LandSource {
                                    location: Some(
                                        tricerules_proto::ruled::v1::land_source::Location::
                                            HandIndex(land.hand_index),
                                    ),
                                    ..Default::default()
                                }),
                                face_index: land.face_index,
                                ..Default::default()
                            })),
                        },
                    );
                    kaya_capture_battlefield(&latest, 17, &mut battlefield);
                    continue;
                }

                let jace_spell = actions
                    .hand_actions
                    .iter()
                    .find(|action| {
                        action.kind == HandActionKind::HandActionCastSpell as i32
                            && action.card_name == JACE_NAME
                    })
                    .cloned();
                let blue_mana_offers = kaya_available_mana_abilities(&battlefield)
                    .iter()
                    .filter(|ability| kaya_output_has_color(&ability.3, 'U'))
                    .count();
                if let Some(spell) = jace_spell.filter(|_| blue_mana_offers >= 3) {
                    latest = kaya_player_command(
                        &mut session,
                        17,
                        RuledCommand {
                            cmd: Some(Cmd::BeginSpellCast(BeginSpellCast {
                                announcement: Some(
                                    tricerules_proto::ruled::v1::SpellCastAnnouncement {
                                        source: Some(CastSource {
                                            location: Some(
                                                tricerules_proto::ruled::v1::cast_source::Location::
                                                    HandIndex(spell.hand_index),
                                            ),
                                            expected_zone_change_generation: None,
                                        }),
                                        face_index: spell.face_index,
                                        cast_method: CastMethod::Normal as i32,
                                        ..Default::default()
                                    },
                                ),
                            })),
                        },
                    );
                    kaya_capture_battlefield(&latest, 17, &mut battlefield);

                    let mut produced = PaymentMana::default();
                    for _ in 0..3 {
                        let (source_object_id, generation, ability_index, output) =
                            kaya_available_mana_abilities(&battlefield)
                                .into_iter()
                                .find(|ability| kaya_output_has_color(&ability.3, 'U'))
                                .expect("current Island view offers the required blue mana");
                        assert!(kaya_output_has_color(&output, 'U'));
                        produced.u += 1;
                        latest = kaya_player_command(
                            &mut session,
                            17,
                            RuledCommand {
                                cmd: Some(Cmd::ActivateAbility(ActivateAbility {
                                    source_object_id,
                                    ability_index,
                                    source_zone: AbilitySourceZone::Battlefield as i32,
                                    expected_zone_change_generation: generation,
                                    ..Default::default()
                                })),
                            },
                        );
                        kaya_capture_battlefield(&latest, 17, &mut battlefield);
                    }

                    let pending = kaya_batch(&latest)
                        .legal_by_player
                        .get(&17)
                        .and_then(|actions| actions.pending_spell_cast.as_ref())
                        .expect("current cast-payment offer survives mana activation");
                    let payment = pending
                        .payment_preview
                        .as_ref()
                        .expect("pending cast publishes its current payment preview");
                    let transaction_id = pending.transaction_id;
                    let revision = payment.revision;
                    let payment_query = session
                        .process(&IpcEnvelope {
                            msg: Some(Msg::PaymentQuery(PaymentQuery {
                                player_id: 17,
                                preview: Some(PreviewPayment {
                                    transaction_id,
                                    revision,
                                    commit_spell_cast: Some(CommitSpellCast {
                                        transaction_id,
                                        payment: Some(PaymentSelection {
                                            expected_state_revision: revision,
                                            mana: Some(produced),
                                            ..Default::default()
                                        }),
                                        ..Default::default()
                                    }),
                                    ..Default::default()
                                }),
                            })),
                        })
                        .expect("payment query keeps the session open");
                    assert!(
                        payment_query.ok,
                        "Jace payment preview: {}",
                        payment_query.error
                    );
                    let preview = payment_query
                        .batch
                        .and_then(|batch| batch.payment_preview)
                        .expect("PaymentQuery returns a current payment preview");
                    assert!(
                        preview.valid && preview.complete,
                        "Jace payment: {preview:?}"
                    );
                    latest = kaya_player_command(
                        &mut session,
                        17,
                        RuledCommand {
                            cmd: Some(Cmd::CommitSpellCast(CommitSpellCast {
                                transaction_id,
                                payment: preview.selection,
                                restricted_mana: preview.restricted_mana,
                            })),
                        },
                    );
                    kaya_capture_battlefield(&latest, 17, &mut battlefield);

                    for _ in 0..8 {
                        if jace_from_view(&battlefield).is_some() {
                            jace_resolved = true;
                            break;
                        }
                        latest = kaya_pass_priority(&mut session, latest);
                        kaya_capture_battlefield(&latest, 17, &mut battlefield);
                    }
                    assert!(
                        jace_resolved,
                        "Jace resolves through ordinary priority passes"
                    );
                    break;
                }
            }

            latest = kaya_pass_priority(&mut session, latest);
            kaya_capture_battlefield(&latest, 17, &mut battlefield);
        }
        assert!(jace_resolved, "normal offers eventually cast Jace");

        let jace = jace_from_view(&battlefield).expect("current view publishes Jace");
        let prior_battlefield_ids = battlefield
            .iter()
            .map(|object| object.object_id)
            .collect::<Vec<_>>();
        let minus_two = jace
            .activated_abilities
            .iter()
            .find(|ability| {
                ability.presentation.as_ref().is_some_and(|presentation| {
                    presentation.path.iter().any(|component| {
                        component.kind == PresentationPathKind::Ability as i32
                            && component.id == "activated_02"
                    })
                })
            })
            .expect("current Jace view identifies -2 by stable ability ID");
        assert!(minus_two.activatable, "Jace's current -2 offer is legal");
        latest = kaya_player_command(
            &mut session,
            17,
            RuledCommand {
                cmd: Some(Cmd::ActivateAbility(ActivateAbility {
                    source_object_id: jace.object_id,
                    ability_index: minus_two.ability_index,
                    source_zone: AbilitySourceZone::Battlefield as i32,
                    expected_zone_change_generation: jace.zone_change_generation,
                    ..Default::default()
                })),
            },
        );
        kaya_capture_battlefield(&latest, 17, &mut battlefield);

        let mut illusion = None;
        for _ in 0..8 {
            let new_objects = battlefield
                .iter()
                .filter(|object| !prior_battlefield_ids.contains(&object.object_id))
                .collect::<Vec<_>>();
            if !new_objects.is_empty() {
                assert_eq!(new_objects.len(), 1, "Jace's -2 creates one Illusion");
                assert!(new_objects[0].is_creature);
                assert_eq!(new_objects[0].power, 2);
                assert_eq!(new_objects[0].toughness, 2);
                illusion = Some((*new_objects[0]).clone());
                break;
            }
            latest = kaya_pass_priority(&mut session, latest);
            kaya_capture_battlefield(&latest, 17, &mut battlefield);
        }
        let illusion = illusion.expect("ordinary priority resolves Jace's -2 Illusion");

        let mut reached_opponent_main = false;
        let mut reached_next_active_main = false;
        for _ in 0..256 {
            let (active_player_id, priority_player_id, turn_step) = {
                let state = &session.engine.as_ref().unwrap().state;
                (
                    state.players[state.active_player_idx].id,
                    state.priority_player_id(),
                    state.turn_step,
                )
            };
            if active_player_id == 29 && turn_step == tricerules_core::TurnStep::Main1 {
                reached_opponent_main = true;
            }
            if reached_opponent_main
                && active_player_id == 17
                && priority_player_id == 17
                && turn_step == tricerules_core::TurnStep::Main1
            {
                reached_next_active_main = true;
                break;
            }
            latest = kaya_pass_priority(&mut session, latest);
            kaya_capture_battlefield(&latest, 17, &mut battlefield);
        }
        assert!(
            reached_next_active_main,
            "Jace reaches its controller's next main phase"
        );

        if let Some(land) = kaya_batch(&latest)
            .legal_by_player
            .get(&17)
            .expect("active player receives next-turn legal actions")
            .hand_actions
            .iter()
            .find(|action| action.kind == HandActionKind::HandActionPlayLand as i32)
            .cloned()
        {
            latest = kaya_player_command(
                &mut session,
                17,
                RuledCommand {
                    cmd: Some(Cmd::PlayLand(PlayLand {
                        source: Some(LandSource {
                            location: Some(
                                tricerules_proto::ruled::v1::land_source::Location::HandIndex(
                                    land.hand_index,
                                ),
                            ),
                            ..Default::default()
                        }),
                        face_index: land.face_index,
                        ..Default::default()
                    })),
                },
            );
            kaya_capture_battlefield(&latest, 17, &mut battlefield);
        }

        let jace = jace_from_view(&battlefield).expect("current view still publishes Jace");
        let plus_one = jace
            .activated_abilities
            .iter()
            .find(|ability| {
                ability.presentation.as_ref().is_some_and(|presentation| {
                    presentation.path.iter().any(|component| {
                        component.kind == PresentationPathKind::Ability as i32
                            && component.id == "activated_01"
                    })
                })
            })
            .expect("current Jace view identifies +1 by stable ability ID");
        assert!(
            plus_one.activatable,
            "Jace's current +1 offer is legal next turn"
        );
        latest = kaya_player_command(
            &mut session,
            17,
            RuledCommand {
                cmd: Some(Cmd::ActivateAbility(ActivateAbility {
                    source_object_id: jace.object_id,
                    ability_index: plus_one.ability_index,
                    source_zone: AbilitySourceZone::Battlefield as i32,
                    expected_zone_change_generation: jace.zone_change_generation,
                    ..Default::default()
                })),
            },
        );
        kaya_capture_battlefield(&latest, 17, &mut battlefield);
        for _ in 0..2 {
            latest = kaya_pass_priority(&mut session, latest);
            kaya_capture_battlefield(&latest, 17, &mut battlefield);
        }

        let mut reached_attackers = false;
        for _ in 0..128 {
            let state = &session.engine.as_ref().unwrap().state;
            if state.players[state.active_player_idx].id == 17
                && state.turn_step == tricerules_core::TurnStep::DeclareAttackers
                && state.priority_player_id() == 17
            {
                reached_attackers = true;
                break;
            }
            latest = kaya_pass_priority(&mut session, latest);
            kaya_capture_battlefield(&latest, 17, &mut battlefield);
        }
        assert!(
            reached_attackers,
            "ordinary priority reaches Declare Attackers"
        );

        let current_illusion = battlefield
            .iter()
            .find(|object| object.object_id == illusion.object_id)
            .expect("current battlefield view still publishes the Illusion");
        assert_eq!(
            current_illusion.zone_change_generation,
            illusion.zone_change_generation
        );
        let attack = kaya_batch(&latest)
            .legal_by_player
            .get(&17)
            .expect("active player receives current attack offers")
            .legal_attack_assignments
            .iter()
            .find(|assignment| {
                assignment.attacker_object_id == current_illusion.object_id
                    && assignment.defending_player_id == 29
            })
            .cloned()
            .expect("current legal attack offer includes the ready Illusion");
        assert_eq!(
            attack.attacker_zone_change_generation,
            current_illusion.zone_change_generation
        );
        latest = kaya_player_command(
            &mut session,
            17,
            RuledCommand {
                cmd: Some(Cmd::DeclareAttackers(
                    tricerules_proto::ruled::v1::DeclareAttackers {
                        assignments: vec![attack],
                    },
                )),
            },
        );

        let mut discard_offer = kaya_choice(&latest, ChoiceKind::HandCards);
        for _ in 0..64 {
            if discard_offer.is_some() {
                break;
            }
            latest = kaya_pass_priority(&mut session, latest);
            kaya_capture_battlefield(&latest, 17, &mut battlefield);
            discard_offer = kaya_choice(&latest, ChoiceKind::HandCards);
        }
        let discard_offer =
            discard_offer.expect("Jace's combat-damage trigger reaches its private discard offer");
        assert_eq!(discard_offer.deciding_player_id, 17);
        assert_ne!(discard_offer.resolution_prompt_id, 0);
        assert_eq!(discard_offer.min, 1);
        assert_eq!(discard_offer.max, 1);
        let chosen_object_id = *discard_offer
            .candidate_object_ids
            .first()
            .expect("current private discard offer has a live candidate");
        latest = kaya_player_command(
            &mut session,
            17,
            RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    resolution_prompt_id: discard_offer.resolution_prompt_id,
                    chosen_object_ids: vec![chosen_object_id],
                    decision: ResolutionChoiceDecision::Unspecified as i32,
                    ..Default::default()
                })),
            },
        );
        assert!(
            kaya_choice(&latest, ChoiceKind::HandCards).is_none(),
            "the current engine-side private discard offer resolves"
        );
    }

    fn sorin_pass_priority(session: &mut EngineSession, latest: IpcResponse) -> IpcResponse {
        let (mut player_id, discard_count) = {
            let state = &session
                .engine
                .as_ref()
                .expect("Sorin session has an engine")
                .state;
            let player_id = state.priority_player_id();
            let discard_count = if state.turn_step == tricerules_core::TurnStep::Cleanup
                && !state.cleanup_priority_active
                && state.cleanup_discard_player == Some(player_id)
            {
                // Hand size is public; this reads no opponent card identity.
                state
                    .players
                    .iter()
                    .find(|player| player.id == player_id)
                    .expect("cleanup actor is in the game")
                    .hand
                    .len()
                    .saturating_sub(7)
            } else {
                0
            };
            (player_id, discard_count)
        };
        if discard_count > 0 {
            let legal_actions = kaya_batch(&latest)
                .legal_by_player
                .get(&player_id)
                .expect("cleanup actor receives current legal actions");
            let cleanup_offers = legal_actions
                .hand_actions
                .iter()
                .filter(|offer| {
                    offer.kind == HandActionKind::HandActionCleanupDiscard as i32
                        && (player_id != 17 || offer.card_name != "Sorin, Grim Nemesis")
                })
                .take(discard_count)
                .map(|offer| offer.hand_index)
                .collect::<Vec<_>>();
            assert_eq!(
                cleanup_offers.len(),
                discard_count,
                "cleanup offers include enough cards without discarding Sorin"
            );
            kaya_player_command(
                session,
                player_id,
                RuledCommand {
                    cmd: Some(Cmd::DiscardToHandSize(DiscardToHandSize {
                        hand_card_indices: cleanup_offers,
                    })),
                },
            );
            player_id = session
                .engine
                .as_ref()
                .expect("Sorin session has an engine")
                .state
                .priority_player_id();
        }
        kaya_player_command(
            session,
            player_id,
            RuledCommand {
                cmd: Some(Cmd::PassPriority(PassPriority {})),
            },
        )
    }

    #[test]
    fn sorin_plus_one_normal_session_casts_and_resolves_from_current_offers() {
        const SORIN_NAME: &str = "Sorin, Grim Nemesis";
        const SORIN_CARD_ID: &str = "sorin,_grim_nemesis";

        let make_deck = |player_id: i32, sorin_pilot: bool| {
            let mut mainboard_card_name = if sorin_pilot {
                vec![SORIN_NAME.to_string(); 4]
            } else {
                Vec::new()
            };
            if sorin_pilot {
                mainboard_card_name.extend(vec!["Plains".to_string(); 28]);
                mainboard_card_name.extend(vec!["Swamp".to_string(); 28]);
            } else {
                mainboard_card_name.extend(vec!["Forest".to_string(); 60]);
            }
            PlayerDeck {
                player_id,
                mainboard_card_name,
                commander_setup: Some(CommanderSetup {
                    declaration: Some(commander_setup::Declaration::NoCommander(true)),
                }),
            }
        };
        let player_decks = vec![make_deck(17, true), make_deck(29, false)];
        assert_eq!(player_decks[0].mainboard_card_name.len(), 60);
        assert_eq!(player_decks[1].mainboard_card_name.len(), 60);

        let mut session = EngineSession::new(false);
        let validation = session
            .process(&IpcEnvelope {
                msg: Some(Msg::ValidateDeck(ValidateDeck {
                    card_names: vec![
                        SORIN_NAME.into(),
                        "Plains".into(),
                        "Swamp".into(),
                        "Forest".into(),
                    ],
                })),
            })
            .expect("deck validation leaves the session open");
        assert!(
            validation.ok,
            "complete Sorin decks validate: {}",
            validation.error
        );
        assert!(validation.missing_card_names.is_empty());

        let mut latest = session
            .process(&IpcEnvelope {
                msg: Some(Msg::SessionStart(SessionStart {
                    game_id: 7_281_030,
                    seed: 20_261_030,
                    player_ids: vec![17, 29],
                    player_decks,
                    dev_commands_enabled: false,
                    diagnostic_capture_enabled: false,
                    ..Default::default()
                })),
            })
            .expect("SessionStart keeps the normal game session open");
        assert!(latest.ok, "normal Sorin GameStart: {}", latest.error);
        assert!(!latest.effective_dev_commands_enabled);

        let starting = kaya_starting_player_offer(&latest)
            .expect("GameStart publishes the eligible starting-player offer");
        assert!(starting.eligible_starting_player_ids.contains(&17));
        latest = kaya_player_command(
            &mut session,
            starting.deciding_player_id,
            RuledCommand {
                cmd: Some(Cmd::ChooseStartingPlayer(ChooseStartingPlayer {
                    starting_player_id: 17,
                })),
            },
        );
        while let Some(opening) = kaya_mulligan_offer(&latest) {
            latest = kaya_player_command(
                &mut session,
                opening.deciding_player_id,
                RuledCommand {
                    cmd: Some(Cmd::Mulligan(MulliganDecision { keep: true })),
                },
            );
        }
        for _ in 0..4 {
            latest = sorin_pass_priority(&mut session, latest);
        }
        {
            let state = &session.engine.as_ref().unwrap().state;
            assert_eq!(state.turn_step, tricerules_core::TurnStep::Main1);
            assert_eq!(state.players[state.active_player_idx].id, 17);
        }

        let mut battlefield = Vec::new();
        kaya_capture_battlefield(&latest, 17, &mut battlefield);
        let sorin_from_view = |objects: &[BattlefieldObject]| {
            objects
                .iter()
                .find(|object| object.card_id == SORIN_CARD_ID)
                .cloned()
        };
        let mut sorin_cast = false;
        for _ in 0..2_048 {
            let (active_player_id, priority_player_id, main_phase) = {
                let state = &session.engine.as_ref().unwrap().state;
                (
                    state.players[state.active_player_idx].id,
                    state.priority_player_id(),
                    state.turn_step == tricerules_core::TurnStep::Main1,
                )
            };

            if active_player_id == 17 && priority_player_id == 17 && main_phase {
                let actions = kaya_batch(&latest)
                    .legal_by_player
                    .get(&17)
                    .expect("active pilot receives current legal actions")
                    .clone();
                let mana_sources = kaya_available_mana_abilities(&battlefield);
                let white_sources = mana_sources
                    .iter()
                    .filter(|ability| kaya_output_has_color(&ability.3, 'W'))
                    .count();
                let black_sources = mana_sources
                    .iter()
                    .filter(|ability| kaya_output_has_color(&ability.3, 'B'))
                    .count();
                let preferred_land = if white_sources == 0 {
                    Some("Plains")
                } else if black_sources == 0 {
                    Some("Swamp")
                } else {
                    None
                };
                let land = actions
                    .hand_actions
                    .iter()
                    .find(|action| {
                        action.kind == HandActionKind::HandActionPlayLand as i32
                            && preferred_land.is_none_or(|name| action.card_name == name)
                    })
                    .cloned()
                    .or_else(|| {
                        actions
                            .hand_actions
                            .iter()
                            .find(|action| action.kind == HandActionKind::HandActionPlayLand as i32)
                            .cloned()
                    });
                if let Some(land) = land {
                    latest = kaya_player_command(
                        &mut session,
                        17,
                        RuledCommand {
                            cmd: Some(Cmd::PlayLand(PlayLand {
                                source: Some(LandSource {
                                    location: Some(
                                        tricerules_proto::ruled::v1::land_source::Location::
                                            HandIndex(land.hand_index),
                                    ),
                                    ..Default::default()
                                }),
                                face_index: land.face_index,
                                ..Default::default()
                            })),
                        },
                    );
                    kaya_capture_battlefield(&latest, 17, &mut battlefield);
                    continue;
                }

                let sorin_spell = actions
                    .hand_actions
                    .iter()
                    .find(|action| {
                        action.kind == HandActionKind::HandActionCastSpell as i32
                            && action.card_name == SORIN_NAME
                    })
                    .cloned();
                let mana_sources = kaya_available_mana_abilities(&battlefield);
                let white_sources = mana_sources
                    .iter()
                    .filter(|ability| kaya_output_has_color(&ability.3, 'W'))
                    .count();
                let black_sources = mana_sources
                    .iter()
                    .filter(|ability| kaya_output_has_color(&ability.3, 'B'))
                    .count();
                if let Some(spell) = sorin_spell
                    .filter(|_| mana_sources.len() >= 6 && white_sources >= 1 && black_sources >= 1)
                {
                    latest = kaya_player_command(
                        &mut session,
                        17,
                        RuledCommand {
                            cmd: Some(Cmd::BeginSpellCast(BeginSpellCast {
                                announcement: Some(
                                    tricerules_proto::ruled::v1::SpellCastAnnouncement {
                                        source: Some(CastSource {
                                            location: Some(
                                                tricerules_proto::ruled::v1::cast_source::Location::
                                                    HandIndex(spell.hand_index),
                                            ),
                                            expected_zone_change_generation: None,
                                        }),
                                        face_index: spell.face_index,
                                        cast_method: CastMethod::Normal as i32,
                                        ..Default::default()
                                    },
                                ),
                            })),
                        },
                    );
                    let mut selected_sources = Vec::new();
                    for color in ['W', 'B'] {
                        selected_sources.push(
                            mana_sources
                                .iter()
                                .find(|source| {
                                    kaya_output_has_color(&source.3, color)
                                        && !selected_sources.iter().any(
                                            |selected: &(u32, u64, u32, String)| {
                                                selected.0 == source.0
                                                    && selected.1 == source.1
                                                    && selected.2 == source.2
                                            },
                                        )
                                })
                                .expect("current lands include each required Sorin color")
                                .clone(),
                        );
                    }
                    for source in &mana_sources {
                        if selected_sources.len() == 6 {
                            break;
                        }
                        if !selected_sources.iter().any(|selected| {
                            selected.0 == source.0
                                && selected.1 == source.1
                                && selected.2 == source.2
                        }) {
                            selected_sources.push(source.clone());
                        }
                    }
                    assert_eq!(selected_sources.len(), 6);
                    let mut produced = PaymentMana::default();
                    for (source_object_id, generation, ability_index, output) in selected_sources {
                        let current_source = kaya_available_mana_abilities(&battlefield)
                            .into_iter()
                            .find(|source| {
                                source.0 == source_object_id
                                    && source.1 == generation
                                    && source.2 == ability_index
                            })
                            .expect("current battlefield offer retains this untapped mana source");
                        assert_eq!(current_source.3, output);
                        if kaya_output_has_color(&output, 'W') {
                            produced.w += 1;
                        } else if kaya_output_has_color(&output, 'B') {
                            produced.b += 1;
                        } else {
                            panic!("Sorin test deck offered an unexpected mana color: {output}");
                        }
                        latest = kaya_player_command(
                            &mut session,
                            17,
                            RuledCommand {
                                cmd: Some(Cmd::ActivateAbility(ActivateAbility {
                                    source_object_id,
                                    ability_index,
                                    source_zone: AbilitySourceZone::Battlefield as i32,
                                    expected_zone_change_generation: generation,
                                    ..Default::default()
                                })),
                            },
                        );
                        kaya_capture_battlefield(&latest, 17, &mut battlefield);
                    }

                    let pending = kaya_batch(&latest)
                        .legal_by_player
                        .get(&17)
                        .and_then(|actions| actions.pending_spell_cast.as_ref())
                        .expect("current Sorin payment offer survives mana activation");
                    let payment = pending
                        .payment_preview
                        .as_ref()
                        .expect("pending Sorin cast publishes its payment preview");
                    let transaction_id = pending.transaction_id;
                    let revision = payment.revision;
                    let payment_query = session
                        .process(&IpcEnvelope {
                            msg: Some(Msg::PaymentQuery(PaymentQuery {
                                player_id: 17,
                                preview: Some(PreviewPayment {
                                    transaction_id,
                                    revision,
                                    commit_spell_cast: Some(CommitSpellCast {
                                        transaction_id,
                                        payment: Some(PaymentSelection {
                                            expected_state_revision: revision,
                                            mana: Some(produced),
                                            ..Default::default()
                                        }),
                                        ..Default::default()
                                    }),
                                    ..Default::default()
                                }),
                            })),
                        })
                        .expect("Sorin payment query keeps the session open");
                    assert!(
                        payment_query.ok,
                        "Sorin payment preview: {}",
                        payment_query.error
                    );
                    let preview = payment_query
                        .batch
                        .and_then(|batch| batch.payment_preview)
                        .expect("PaymentQuery returns a current Sorin payment preview");
                    assert!(
                        preview.valid && preview.complete,
                        "Sorin payment: {preview:?}"
                    );
                    latest = kaya_player_command(
                        &mut session,
                        17,
                        RuledCommand {
                            cmd: Some(Cmd::CommitSpellCast(CommitSpellCast {
                                transaction_id,
                                payment: preview.selection,
                                restricted_mana: preview.restricted_mana,
                            })),
                        },
                    );
                    kaya_capture_battlefield(&latest, 17, &mut battlefield);
                    for _ in 0..8 {
                        if sorin_from_view(&battlefield).is_some() {
                            break;
                        }
                        latest = sorin_pass_priority(&mut session, latest);
                        kaya_capture_battlefield(&latest, 17, &mut battlefield);
                    }
                    assert!(
                        sorin_from_view(&battlefield).is_some(),
                        "Sorin resolves through ordinary priority passes"
                    );
                    sorin_cast = true;
                    break;
                }
            }

            latest = sorin_pass_priority(&mut session, latest);
            kaya_capture_battlefield(&latest, 17, &mut battlefield);
        }
        assert!(sorin_cast, "ordinary offers eventually cast Sorin");

        let sorin =
            sorin_from_view(&battlefield).expect("current battlefield view publishes Sorin");
        let plus_one = sorin
            .activated_abilities
            .iter()
            .find(|ability| {
                ability.presentation.as_ref().is_some_and(|presentation| {
                    presentation.path.iter().any(|component| {
                        component.kind == PresentationPathKind::Ability as i32
                            && component.id == "activated_01"
                    })
                })
            })
            .expect("current Sorin view identifies +1 by stable ability ID");
        assert!(plus_one.activatable, "Sorin's current +1 offer is legal");
        let plus_one_index = plus_one.ability_index;
        let source_object_id = sorin.object_id;
        let source_generation = sorin.zone_change_generation;
        let (controller_life_before, opponent_life_before, cards_drawn_before) = {
            let engine = session.engine.as_ref().unwrap();
            (
                engine.state.players[engine.state.player_idx(17).unwrap()].life,
                engine.state.players[engine.state.player_idx(29).unwrap()].life,
                engine.state.turn_history.current.player(0).cards_drawn,
            )
        };
        latest = kaya_player_command(
            &mut session,
            17,
            RuledCommand {
                cmd: Some(Cmd::ActivateAbility(ActivateAbility {
                    source_object_id,
                    ability_index: plus_one_index,
                    source_zone: AbilitySourceZone::Battlefield as i32,
                    expected_zone_change_generation: source_generation,
                    ..Default::default()
                })),
            },
        );
        kaya_capture_battlefield(&latest, 17, &mut battlefield);
        assert_eq!(
            sorin_from_view(&battlefield).unwrap().loyalty,
            7,
            "the ordinary activation offer applies Sorin's +1 cost"
        );

        let mut revealed = None;
        for _ in 0..4 {
            latest = sorin_pass_priority(&mut session, latest);
            for event in &kaya_batch(&latest).events {
                if let Some(Ev::CardsRevealed(cards)) = event.ev.as_ref() {
                    assert_eq!(cards.cards.len(), 1);
                    revealed = cards.cards.first().map(|card| {
                        (
                            card.object_id,
                            card.card_id.clone(),
                            cards.zone_owner_player_id,
                        )
                    });
                    break;
                }
            }
            if revealed.is_some() {
                break;
            }
        }
        let (revealed_object_id, revealed_card_id, library_owner) =
            revealed.expect("the public reveal event supplies the expected card identity");
        assert_eq!(library_owner, 17);
        let revealed_definition = tricerules_cards::registry::global()
            .get(&revealed_card_id)
            .expect("the public reveal identifies a registered card");
        let expected_mana_value = revealed_definition.primary_face().mana_cost.mana_value();
        {
            let engine = session.engine.as_ref().unwrap();
            assert_eq!(
                engine.state.players[engine.state.player_idx(17).unwrap()].life,
                controller_life_before,
                "Sorin's +1 does not change its controller's life"
            );
            assert_eq!(
                engine.state.players[engine.state.player_idx(29).unwrap()].life,
                opponent_life_before - expected_mana_value as i32,
                "the opponent loses the publicly revealed card's mana value"
            );
            assert_eq!(
                engine.state.turn_history.current.player(0).cards_drawn,
                cards_drawn_before,
                "putting the revealed card into hand is not a draw"
            );
        }
        let final_view = kaya_batch(&latest)
            .events
            .iter()
            .rev()
            .find_map(|event| match event.ev.as_ref() {
                Some(Ev::ZoneView(view)) => Some(view),
                _ => None,
            })
            .expect("the resolved +1 publishes a final zone view");
        let controller_view = final_view
            .per_player
            .iter()
            .find(|view| view.player_id == 17)
            .expect("the controller receives its private hand view");
        assert!(controller_view
            .hand_cards
            .iter()
            .any(|card| card.object_id == revealed_object_id));
    }

    #[test]
    fn teferi_master_of_time_casts_and_resolves_plus_one_from_normal_offers() {
        const NAME: &str = "Teferi, Master of Time";
        const CARD_ID: &str = "teferi,_master_of_time";
        let make_deck = |player_id: i32, pilot: bool| {
            let count = if pilot { 4 } else { 0 };
            let mut cards = vec![NAME.to_string(); count];
            let lands = 60 - cards.len();
            cards.extend(vec![
                if pilot { "Island" } else { "Forest" }.to_string();
                lands
            ]);
            PlayerDeck {
                player_id,
                mainboard_card_name: cards,
                commander_setup: Some(CommanderSetup {
                    declaration: Some(commander_setup::Declaration::NoCommander(true)),
                }),
            }
        };
        let player_decks = vec![make_deck(17, true), make_deck(29, false)];
        assert_eq!(player_decks[0].mainboard_card_name.len(), 60);

        let mut session = EngineSession::new(false);
        let validation = session
            .process(&IpcEnvelope {
                msg: Some(Msg::ValidateDeck(ValidateDeck {
                    card_names: vec![NAME.into(), "Island".into(), "Forest".into()],
                })),
            })
            .expect("deck validation keeps the normal session open");
        assert!(
            validation.ok,
            "Teferi deck validation: {}",
            validation.error
        );
        assert!(validation.missing_card_names.is_empty());
        let mut latest = session
            .process(&IpcEnvelope {
                msg: Some(Msg::SessionStart(SessionStart {
                    game_id: 7_281_032,
                    seed: 20_261_032,
                    player_ids: vec![17, 29],
                    player_decks,
                    dev_commands_enabled: false,
                    diagnostic_capture_enabled: false,
                    ..Default::default()
                })),
            })
            .expect("normal SessionStart keeps the session open");
        assert!(latest.ok, "Teferi GameStart: {}", latest.error);
        assert!(!latest.effective_dev_commands_enabled);
        let starting = kaya_starting_player_offer(&latest).expect("starting-player offer");
        assert!(starting.eligible_starting_player_ids.contains(&17));
        latest = kaya_player_command(
            &mut session,
            starting.deciding_player_id,
            RuledCommand {
                cmd: Some(Cmd::ChooseStartingPlayer(ChooseStartingPlayer {
                    starting_player_id: 17,
                })),
            },
        );
        while let Some(opening) = kaya_mulligan_offer(&latest) {
            latest = kaya_player_command(
                &mut session,
                opening.deciding_player_id,
                RuledCommand {
                    cmd: Some(Cmd::Mulligan(MulliganDecision { keep: true })),
                },
            );
        }
        for _ in 0..4 {
            latest = kaya_pass_priority(&mut session, latest);
        }
        let state = &session.engine.as_ref().unwrap().state;
        assert_eq!(state.turn_step, tricerules_core::TurnStep::Main1);
        assert_eq!(state.players[state.active_player_idx].id, 17);

        let mut battlefield = Vec::new();
        kaya_capture_battlefield(&latest, 17, &mut battlefield);
        let teferi_from_view = |view: &[BattlefieldObject]| {
            view.iter()
                .find(|object| object.card_id == CARD_ID)
                .cloned()
        };
        let mut cast = false;
        for _ in 0..4096 {
            let (active, priority, main1) = {
                let state = &session.engine.as_ref().unwrap().state;
                (
                    state.players[state.active_player_idx].id,
                    state.priority_player_id(),
                    state.turn_step == tricerules_core::TurnStep::Main1,
                )
            };
            if active == 17 && priority == 17 && main1 {
                let actions = kaya_batch(&latest)
                    .legal_by_player
                    .get(&17)
                    .expect("current pilot actions")
                    .clone();
                if let Some(land) = actions
                    .hand_actions
                    .iter()
                    .find(|offer| offer.kind == HandActionKind::HandActionPlayLand as i32)
                    .cloned()
                {
                    latest = kaya_player_command(&mut session, 17, RuledCommand {
                        cmd: Some(Cmd::PlayLand(PlayLand {
                            source: Some(LandSource {
                                location: Some(tricerules_proto::ruled::v1::land_source::Location::HandIndex(land.hand_index)),
                                ..Default::default()
                            }), face_index: land.face_index, ..Default::default()
                        })),
                    });
                    kaya_capture_battlefield(&latest, 17, &mut battlefield);
                    continue;
                }
                let spell = actions
                    .hand_actions
                    .iter()
                    .find(|offer| {
                        offer.kind == HandActionKind::HandActionCastSpell as i32
                            && offer.card_name == NAME
                    })
                    .cloned();
                let mana = kaya_available_mana_abilities(&battlefield)
                    .into_iter()
                    .filter(|source| kaya_output_has_color(&source.3, 'U'))
                    .collect::<Vec<_>>();
                if let Some(spell) = spell.filter(|_| mana.len() >= 4) {
                    latest = kaya_player_command(&mut session, 17, RuledCommand {
                        cmd: Some(Cmd::BeginSpellCast(BeginSpellCast {
                            announcement: Some(tricerules_proto::ruled::v1::SpellCastAnnouncement {
                                source: Some(CastSource {
                                    location: Some(tricerules_proto::ruled::v1::cast_source::Location::HandIndex(spell.hand_index)),
                                    expected_zone_change_generation: None,
                                }),
                                face_index: spell.face_index,
                                cast_method: CastMethod::Normal as i32,
                                ..Default::default()
                            }),
                        })),
                    });
                    let mut produced = PaymentMana::default();
                    for (object_id, generation, ability_index, output) in mana.into_iter().take(4) {
                        let current_source = kaya_available_mana_abilities(&battlefield)
                            .into_iter()
                            .find(|source| {
                                source.0 == object_id
                                    && source.1 == generation
                                    && source.2 == ability_index
                            })
                            .expect("current Teferi view retains this untapped Island offer");
                        assert_eq!(current_source.3, output);
                        assert!(
                            kaya_output_has_color(&output, 'U'),
                            "Island offers blue mana: {output}"
                        );
                        produced.u += 1;
                        latest = kaya_player_command(
                            &mut session,
                            17,
                            RuledCommand {
                                cmd: Some(Cmd::ActivateAbility(ActivateAbility {
                                    source_object_id: object_id,
                                    ability_index,
                                    source_zone: AbilitySourceZone::Battlefield as i32,
                                    expected_zone_change_generation: generation,
                                    ..Default::default()
                                })),
                            },
                        );
                        kaya_capture_battlefield(&latest, 17, &mut battlefield);
                    }
                    assert_eq!(produced.u, 4);
                    let pending = kaya_batch(&latest).legal_by_player[&17]
                        .pending_spell_cast
                        .as_ref()
                        .expect("current Teferi cast retains its payment transaction");
                    let transaction_id = pending.transaction_id;
                    let revision = pending
                        .payment_preview
                        .as_ref()
                        .expect("payment preview")
                        .revision;
                    let response = session
                        .process(&IpcEnvelope {
                            msg: Some(Msg::PaymentQuery(PaymentQuery {
                                player_id: 17,
                                preview: Some(PreviewPayment {
                                    transaction_id,
                                    revision,
                                    commit_spell_cast: Some(CommitSpellCast {
                                        transaction_id,
                                        payment: Some(PaymentSelection {
                                            expected_state_revision: revision,
                                            mana: Some(produced),
                                            ..Default::default()
                                        }),
                                        ..Default::default()
                                    }),
                                    ..Default::default()
                                }),
                            })),
                        })
                        .expect("PaymentQuery keeps the Teferi session open");
                    assert!(response.ok, "Teferi payment preview: {}", response.error);
                    let preview = response
                        .batch
                        .and_then(|batch| batch.payment_preview)
                        .expect("PaymentQuery returns Teferi's preview");
                    assert!(
                        preview.valid && preview.complete,
                        "Teferi payment: {preview:?}"
                    );
                    latest = kaya_player_command(
                        &mut session,
                        17,
                        RuledCommand {
                            cmd: Some(Cmd::CommitSpellCast(CommitSpellCast {
                                transaction_id,
                                payment: preview.selection,
                                restricted_mana: preview.restricted_mana,
                            })),
                        },
                    );
                    for _ in 0..8 {
                        kaya_capture_battlefield(&latest, 17, &mut battlefield);
                        if teferi_from_view(&battlefield).is_some() {
                            cast = true;
                            break;
                        }
                        latest = kaya_pass_priority(&mut session, latest);
                    }
                    assert!(cast, "Teferi resolves through ordinary priority passes");
                    break;
                }
            }
            latest = kaya_pass_priority(&mut session, latest);
            kaya_capture_battlefield(&latest, 17, &mut battlefield);
        }
        assert!(cast, "normal offers eventually cast Teferi");
        let teferi = teferi_from_view(&battlefield).expect("Teferi is on the current battlefield");
        assert_eq!(teferi.loyalty, 3);
        let plus_one = teferi
            .activated_abilities
            .iter()
            .find(|ability| {
                ability.presentation.as_ref().is_some_and(|presentation| {
                    presentation.path.iter().any(|part| {
                        part.kind == PresentationPathKind::Ability as i32
                            && part.id == "activated_01"
                    })
                })
            })
            .expect("current Teferi view identifies +1 by stable ability ID");
        assert!(
            plus_one.activatable,
            "current engine offer makes Teferi's +1 legal"
        );
        let (object_id, generation, ability_index) = (
            teferi.object_id,
            teferi.zone_change_generation,
            plus_one.ability_index,
        );
        let (hand_before, library_before, draws_before) = {
            let engine = session.engine.as_ref().unwrap();
            let player = engine.state.player_idx(17).unwrap();
            (
                engine.state.players[player].hand.len(),
                engine.state.players[player].library.len(),
                engine.state.turn_history.current.player(17).cards_drawn,
            )
        };
        latest = kaya_player_command(
            &mut session,
            17,
            RuledCommand {
                cmd: Some(Cmd::ActivateAbility(ActivateAbility {
                    source_object_id: object_id,
                    ability_index,
                    source_zone: AbilitySourceZone::Battlefield as i32,
                    expected_zone_change_generation: generation,
                    ..Default::default()
                })),
            },
        );
        kaya_capture_battlefield(&latest, 17, &mut battlefield);
        assert_eq!(teferi_from_view(&battlefield).unwrap().loyalty, 4);
        assert_eq!(
            session
                .engine
                .as_ref()
                .unwrap()
                .state
                .stack
                .last()
                .and_then(|item| item.source_permanent_id),
            Some(object_id)
        );
        let mut choice = kaya_choice(&latest, ChoiceKind::HandCards);
        for _ in 0..8 {
            if choice.is_some() {
                break;
            }
            latest = kaya_pass_priority(&mut session, latest);
            kaya_capture_battlefield(&latest, 17, &mut battlefield);
            choice = kaya_choice(&latest, ChoiceKind::HandCards);
        }
        let choice = choice.expect("+1 draw reaches its private mandatory discard offer");
        assert_eq!(choice.deciding_player_id, 17);
        assert_ne!(choice.resolution_prompt_id, 0);
        assert_eq!((choice.min, choice.max), (1, 1));
        let discarded = *choice
            .candidate_object_ids
            .first()
            .expect("current discard has a candidate");
        {
            let engine = session.engine.as_ref().unwrap();
            let player = engine.state.player_idx(17).unwrap();
            assert_eq!(engine.state.players[player].hand.len(), hand_before + 1);
            assert_eq!(
                engine.state.players[player].library.len(),
                library_before - 1
            );
            assert_eq!(
                engine.state.turn_history.current.player(17).cards_drawn,
                draws_before + 1
            );
            assert!(engine.state.players[player].hand.contains(&discarded));
        }
        latest = kaya_player_command(
            &mut session,
            17,
            RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    resolution_prompt_id: choice.resolution_prompt_id,
                    chosen_object_ids: vec![discarded],
                    decision: ResolutionChoiceDecision::Unspecified as i32,
                    ..Default::default()
                })),
            },
        );
        assert!(kaya_choice(&latest, ChoiceKind::HandCards).is_none());
        let engine = session.engine.as_ref().unwrap();
        let player = engine.state.player_idx(17).unwrap();
        assert!(engine.state.players[player].graveyard.contains(&discarded));
        assert_eq!(engine.state.players[player].hand.len(), hand_before);
        assert_eq!(
            engine.state.players[player].library.len(),
            library_before - 1
        );
        assert_eq!(
            engine.state.turn_history.current.player(17).cards_drawn,
            draws_before + 1
        );
        assert!(engine.state.stack.is_empty());
        assert!(engine.state.pending_resolution.is_none());
    }

    fn ugin_pass_priority(session: &mut EngineSession, latest: IpcResponse) -> IpcResponse {
        const UGIN_NAME: &str = "Ugin, the Spirit Dragon";
        let mut player_id = session
            .engine
            .as_ref()
            .expect("Ugin session has an engine")
            .state
            .priority_player_id();
        let discard_count = {
            let state = &session.engine.as_ref().unwrap().state;
            if state.turn_step == tricerules_core::TurnStep::Cleanup
                && !state.cleanup_priority_active
                && state.cleanup_discard_player == Some(player_id)
            {
                state
                    .players
                    .iter()
                    .find(|player| player.id == player_id)
                    .expect("cleanup actor is in the game")
                    .hand
                    .len()
                    .saturating_sub(7)
            } else {
                0
            }
        };
        if discard_count > 0 {
            let legal_actions = kaya_batch(&latest)
                .legal_by_player
                .get(&player_id)
                .expect("cleanup actor receives current legal actions");
            let cleanup_offers = legal_actions
                .hand_actions
                .iter()
                .filter(|offer| {
                    offer.kind == HandActionKind::HandActionCleanupDiscard as i32
                        && (player_id != 17 || offer.card_name != UGIN_NAME)
                })
                .take(discard_count)
                .map(|offer| offer.hand_index)
                .collect::<Vec<_>>();
            assert_eq!(
                cleanup_offers.len(),
                discard_count,
                "cleanup offers include enough cards while preserving any Ugin in hand"
            );
            let after_discard = kaya_player_command(
                session,
                player_id,
                RuledCommand {
                    cmd: Some(Cmd::DiscardToHandSize(DiscardToHandSize {
                        hand_card_indices: cleanup_offers,
                    })),
                },
            );
            player_id = session
                .engine
                .as_ref()
                .expect("Ugin session has an engine")
                .state
                .priority_player_id();
            assert!(
                kaya_batch(&after_discard)
                    .legal_by_player
                    .contains_key(&player_id),
                "post-discard response publishes current actions for the new priority player"
            );
        }
        kaya_player_command(
            session,
            player_id,
            RuledCommand {
                cmd: Some(Cmd::PassPriority(PassPriority {})),
            },
        )
    }

    const LUXIOR_NORMAL_NAME: &str = "Luxior, Giada's Gift";
    const LUXIOR_NORMAL_ID: &str = "luxior,_giadas_gift";
    const AJANI_NORMAL_NAME: &str = "Ajani Steadfast";
    const AJANI_NORMAL_ID: &str = "ajani_steadfast";
    const ORNITHOPTER_NORMAL_NAME: &str = "Ornithopter";
    const ORNITHOPTER_NORMAL_ID: &str = "ornithopter";

    #[derive(Default)]
    struct LuxiorNormalHandles {
        luxior: Option<(u32, u64)>,
        ajani: Option<(u32, u64)>,
        ornithopter: Option<(u32, u64)>,
    }

    #[derive(Clone, Copy)]
    struct LuxiorNormalCastPlan<'a> {
        card_name: &'a str,
        expected_total_cost: &'a str,
        white_count: u32,
    }

    #[derive(Clone, Copy)]
    struct LuxiorNormalEquipmentPlan<'a> {
        stable_ability_id: &'a str,
        target_object_id: u32,
        white_count: u32,
        expected_total_cost: &'a str,
    }

    fn luxior_normal_record_handle(
        current: Option<&BattlefieldObject>,
        previous: &mut Option<(u32, u64)>,
        card_name: &str,
    ) {
        let Some(current) = current else {
            return;
        };
        let current_handle = (current.object_id, current.zone_change_generation);
        if let Some(previous_handle) = *previous {
            assert_eq!(
                previous_handle, current_handle,
                "{card_name} keeps its public ObjectId and zone-change generation"
            );
        } else {
            *previous = Some(current_handle);
        }
    }

    fn luxior_normal_refresh_handles(
        battlefield: &[BattlefieldObject],
        handles: &mut LuxiorNormalHandles,
    ) {
        luxior_normal_record_handle(
            battlefield
                .iter()
                .find(|object| object.card_id == LUXIOR_NORMAL_ID),
            &mut handles.luxior,
            LUXIOR_NORMAL_NAME,
        );
        luxior_normal_record_handle(
            battlefield
                .iter()
                .find(|object| object.card_id == AJANI_NORMAL_ID),
            &mut handles.ajani,
            AJANI_NORMAL_NAME,
        );
        luxior_normal_record_handle(
            battlefield
                .iter()
                .find(|object| object.card_id == ORNITHOPTER_NORMAL_ID),
            &mut handles.ornithopter,
            ORNITHOPTER_NORMAL_NAME,
        );
    }

    fn luxior_normal_checkpoint(session: &EngineSession, label: &str, command_ordinal: usize) {
        let state = &session
            .engine
            .as_ref()
            .expect("Luxior session has an engine at each diagnostic checkpoint")
            .state;
        eprintln!(
            "LUXIOR checkpoint={label} ordinal={command_ordinal} turn={} turn_instance={} active={} priority={} step={:?} stack={}",
            state.turn,
            state.turn_instance,
            state.players[state.active_player_idx].id,
            state.priority_player_id(),
            state.turn_step,
            state.stack.len()
        );
    }

    fn luxior_normal_player_command(
        session: &mut EngineSession,
        player_id: i32,
        command: RuledCommand,
        battlefield: &mut Vec<BattlefieldObject>,
        handles: &mut LuxiorNormalHandles,
        accepted_commands: &mut usize,
    ) -> IpcResponse {
        let command_kind = match command.cmd.as_ref() {
            Some(Cmd::ChooseStartingPlayer(_)) => "ChooseStartingPlayer",
            Some(Cmd::Mulligan(_)) => "Mulligan",
            Some(Cmd::PassPriority(_)) => "PassPriority",
            Some(Cmd::DiscardToHandSize(_)) => "DiscardToHandSize",
            Some(Cmd::PlayLand(_)) => "PlayLand",
            Some(Cmd::BeginSpellCast(_)) => "BeginSpellCast",
            Some(Cmd::CommitSpellCast(_)) => "CommitSpellCast",
            Some(Cmd::ActivateAbility(_)) => "ActivateAbility",
            _ => "Other",
        };
        luxior_normal_checkpoint(
            session,
            &format!("before IPC {command_kind} player={player_id}"),
            *accepted_commands + 1,
        );
        let latest = kaya_player_command(session, player_id, command);
        luxior_normal_checkpoint(
            session,
            &format!("after IPC {command_kind} player={player_id}"),
            *accepted_commands + 1,
        );
        *accepted_commands += 1;
        assert!(
            *accepted_commands <= 8_192,
            "Luxior normal pilot stays within its accepted-command bound"
        );
        kaya_capture_battlefield(&latest, 17, battlefield);
        luxior_normal_refresh_handles(battlefield, handles);
        luxior_normal_checkpoint(
            session,
            &format!("after view refresh {command_kind}"),
            *accepted_commands,
        );
        latest
    }

    fn luxior_normal_pass_priority(
        session: &mut EngineSession,
        mut latest: IpcResponse,
        battlefield: &mut Vec<BattlefieldObject>,
        handles: &mut LuxiorNormalHandles,
        accepted_commands: &mut usize,
    ) -> IpcResponse {
        let mut player_id = session
            .engine
            .as_ref()
            .expect("Luxior session has an engine")
            .state
            .priority_player_id();
        let discard_count = session
            .engine
            .as_ref()
            .and_then(|engine| {
                let state = &engine.state;
                let player = state.players.iter().find(|player| player.id == player_id)?;
                Some(
                    if state.turn_step == tricerules_core::TurnStep::Cleanup
                        && !state.cleanup_priority_active
                        && state.cleanup_discard_player == Some(player_id)
                    {
                        player.hand.len().saturating_sub(7)
                    } else {
                        0
                    },
                )
            })
            .expect("priority player is present for the normal cleanup offer");
        if discard_count > 0 {
            let legal_actions = kaya_batch(&latest)
                .legal_by_player
                .get(&player_id)
                .expect("cleanup player receives current legal actions");
            let mut reserved_luxior = false;
            let mut reserved_ajani = false;
            let mut reserved_ornithopter = false;
            let cleanup_offers = legal_actions
                .hand_actions
                .iter()
                .filter(|offer| offer.kind == HandActionKind::HandActionCleanupDiscard as i32)
                .filter_map(|offer| {
                    if player_id == 17 {
                        match offer.card_name.as_str() {
                            LUXIOR_NORMAL_NAME if handles.luxior.is_none() && !reserved_luxior => {
                                reserved_luxior = true;
                                return None;
                            }
                            AJANI_NORMAL_NAME if handles.ajani.is_none() && !reserved_ajani => {
                                reserved_ajani = true;
                                return None;
                            }
                            ORNITHOPTER_NORMAL_NAME
                                if handles.ornithopter.is_none() && !reserved_ornithopter =>
                            {
                                reserved_ornithopter = true;
                                return None;
                            }
                            _ => {}
                        }
                    }
                    Some(offer.hand_index)
                })
                .take(discard_count)
                .collect::<Vec<_>>();
            assert_eq!(
                cleanup_offers.len(),
                discard_count,
                "cleanup discards preserve one current copy of each still-needed pilot spell"
            );
            latest = luxior_normal_player_command(
                session,
                player_id,
                RuledCommand {
                    cmd: Some(Cmd::DiscardToHandSize(DiscardToHandSize {
                        hand_card_indices: cleanup_offers,
                    })),
                },
                battlefield,
                handles,
                accepted_commands,
            );
            player_id = session
                .engine
                .as_ref()
                .expect("Luxior session has an engine after cleanup")
                .state
                .priority_player_id();
            assert!(
                kaya_batch(&latest).legal_by_player.contains_key(&player_id),
                "post-cleanup response publishes current actions for the priority player"
            );
        }
        luxior_normal_player_command(
            session,
            player_id,
            RuledCommand {
                cmd: Some(Cmd::PassPriority(PassPriority {})),
            },
            battlefield,
            handles,
            accepted_commands,
        )
    }

    fn luxior_normal_resolve_stack(
        session: &mut EngineSession,
        mut latest: IpcResponse,
        battlefield: &mut Vec<BattlefieldObject>,
        handles: &mut LuxiorNormalHandles,
        accepted_commands: &mut usize,
    ) -> IpcResponse {
        for _ in 0..8 {
            let stack_empty = session
                .engine
                .as_ref()
                .expect("Luxior session has an engine")
                .state
                .stack
                .is_empty();
            if stack_empty {
                return latest;
            }
            latest = luxior_normal_pass_priority(
                session,
                latest,
                battlefield,
                handles,
                accepted_commands,
            );
        }
        assert!(
            session
                .engine
                .as_ref()
                .expect("Luxior session has an engine")
                .state
                .stack
                .is_empty(),
            "ordinary priority passes resolve the pilot's spell or ability"
        );
        latest
    }

    fn luxior_normal_float_white(
        session: &mut EngineSession,
        mut latest: IpcResponse,
        battlefield: &mut Vec<BattlefieldObject>,
        handles: &mut LuxiorNormalHandles,
        accepted_commands: &mut usize,
        white_count: u32,
        context: &str,
    ) -> (IpcResponse, PaymentMana) {
        luxior_normal_assert_white_pool(session, 0, context);

        let mana_sources = kaya_available_mana_abilities(battlefield)
            .into_iter()
            .filter(|source| kaya_output_has_color(&source.3, 'W'))
            .collect::<Vec<_>>();
        assert!(
            mana_sources.len() >= white_count as usize,
            "current untapped Plains can produce exactly {white_count} white mana for {context}"
        );
        for (index, (object_id, generation, ability_index, output)) in mana_sources
            .into_iter()
            .take(white_count as usize)
            .enumerate()
        {
            let current_source = kaya_available_mana_abilities(battlefield)
                .into_iter()
                .find(|source| {
                    source.0 == object_id && source.1 == generation && source.2 == ability_index
                })
                .expect("latest public battlefield retains this current Plains offer");
            assert_eq!(current_source.3, output);
            assert!(kaya_output_has_color(&output, 'W'));
            latest = luxior_normal_player_command(
                session,
                17,
                RuledCommand {
                    cmd: Some(Cmd::ActivateAbility(ActivateAbility {
                        source_object_id: object_id,
                        ability_index,
                        source_zone: AbilitySourceZone::Battlefield as i32,
                        expected_zone_change_generation: generation,
                        ..Default::default()
                    })),
                },
                battlefield,
                handles,
                accepted_commands,
            );
            let pool = &session
                .engine
                .as_ref()
                .expect("Luxior session has an engine after mana activation")
                .state
                .players
                .iter()
                .find(|player| player.id == 17)
                .expect("pilot remains in the active session")
                .mana_pool;
            assert_eq!(
                pool.white,
                index as u32 + 1,
                "{context} produces white only"
            );
            assert_eq!(pool.blue, 0);
            assert_eq!(pool.black, 0);
            assert_eq!(pool.red, 0);
            assert_eq!(pool.green, 0);
            assert_eq!(pool.colorless, 0);
        }
        let produced = PaymentMana {
            w: white_count,
            ..Default::default()
        };
        (latest, produced)
    }

    fn luxior_normal_public_permanent<'a>(
        battlefield: &'a [BattlefieldObject],
        card_id: &str,
    ) -> &'a BattlefieldObject {
        battlefield
            .iter()
            .find(|object| object.card_id == card_id)
            .unwrap_or_else(|| panic!("current public battlefield includes {card_id}"))
    }

    fn luxior_normal_assert_white_pool(
        session: &EngineSession,
        expected_white: u32,
        context: &str,
    ) {
        let pool = &session
            .engine
            .as_ref()
            .expect("Luxior session has an engine")
            .state
            .players
            .iter()
            .find(|player| player.id == 17)
            .expect("pilot remains in the active session")
            .mana_pool;
        assert_eq!(pool.white, expected_white, "{context} white pool");
        assert_eq!(pool.blue, 0, "{context} has no blue mana");
        assert_eq!(pool.black, 0, "{context} has no black mana");
        assert_eq!(pool.red, 0, "{context} has no red mana");
        assert_eq!(pool.green, 0, "{context} has no green mana");
        assert_eq!(pool.colorless, 0, "{context} has no colorless mana");
    }

    fn luxior_normal_current_ability<'a>(
        permanent: &'a BattlefieldObject,
        stable_id: &str,
    ) -> &'a tricerules_proto::ruled::v1::AbilityInfo {
        permanent
            .activated_abilities
            .iter()
            .find(|ability| {
                ability.presentation.as_ref().is_some_and(|presentation| {
                    presentation.path.iter().any(|component| {
                        component.kind == PresentationPathKind::Ability as i32
                            && component.id == stable_id
                    })
                })
            })
            .unwrap_or_else(|| {
                panic!(
                    "current {} view publishes stable ability {stable_id}",
                    permanent.card_id
                )
            })
    }

    fn luxior_normal_target_group(
        latest: &IpcResponse,
        source_object_id: u32,
        ability_index: u32,
    ) -> &tricerules_proto::ruled::v1::LegalTargetGroup {
        let key = (u64::from(source_object_id) << 32) | u64::from(ability_index);
        let legal_actions = kaya_batch(latest)
            .legal_by_player
            .get(&17)
            .expect("pilot receives current target offers");
        let groups = &legal_actions
            .valid_targets_by_ability
            .get(&key)
            .unwrap_or_else(|| panic!("current source/index {key} publishes target offers"))
            .groups;
        assert_eq!(
            groups.len(),
            1,
            "the current pilot ability has one target group"
        );
        &groups[0]
    }

    fn luxior_normal_target_ref(
        group: &tricerules_proto::ruled::v1::LegalTargetGroup,
        target: &BattlefieldObject,
    ) -> TargetRef {
        assert!(group.valid_permanent_ids.contains(&target.object_id));
        let generation = group
            .valid_permanent_targets
            .iter()
            .find(|candidate| candidate.object_id == target.object_id)
            .map(|candidate| candidate.zone_change_generation)
            .or_else(|| {
                group
                    .valid_permanent_ids
                    .contains(&target.object_id)
                    .then_some(target.zone_change_generation)
            })
            .expect("current public target candidate has its current generation receipt");
        assert_eq!(generation, target.zone_change_generation);
        TargetRef {
            object_id: target.object_id,
            group_index: group.group_index,
            kind: TargetRefKind::Permanent as i32,
            expected_zone_change_generation: Some(generation),
            ..Default::default()
        }
    }

    fn luxior_normal_cast_from_current_offer(
        session: &mut EngineSession,
        mut latest: IpcResponse,
        battlefield: &mut Vec<BattlefieldObject>,
        handles: &mut LuxiorNormalHandles,
        accepted_commands: &mut usize,
        plan: LuxiorNormalCastPlan<'_>,
    ) -> IpcResponse {
        let LuxiorNormalCastPlan {
            card_name,
            expected_total_cost,
            white_count,
        } = plan;
        let spell = kaya_batch(&latest)
            .legal_by_player
            .get(&17)
            .expect("pilot receives current spell offers")
            .hand_actions
            .iter()
            .find(|offer| {
                offer.kind == HandActionKind::HandActionCastSpell as i32
                    && offer.card_name == card_name
            })
            .cloned()
            .unwrap_or_else(|| panic!("current hand offer contains castable {card_name}"));
        assert!(
            session
                .engine
                .as_ref()
                .expect("Luxior session has an engine")
                .state
                .stack
                .is_empty(),
            "cast from the current empty-stack main-phase offer"
        );
        latest = luxior_normal_player_command(
            session,
            17,
            RuledCommand {
                cmd: Some(Cmd::BeginSpellCast(BeginSpellCast {
                    announcement: Some(tricerules_proto::ruled::v1::SpellCastAnnouncement {
                        source: Some(CastSource {
                            location: Some(
                                tricerules_proto::ruled::v1::cast_source::Location::HandIndex(
                                    spell.hand_index,
                                ),
                            ),
                            expected_zone_change_generation: None,
                        }),
                        face_index: spell.face_index,
                        cast_method: CastMethod::Normal as i32,
                        ..Default::default()
                    }),
                })),
            },
            battlefield,
            handles,
            accepted_commands,
        );
        let (next, produced) = luxior_normal_float_white(
            session,
            latest,
            battlefield,
            handles,
            accepted_commands,
            white_count,
            card_name,
        );
        latest = next;

        let pending = kaya_batch(&latest)
            .legal_by_player
            .get(&17)
            .and_then(|actions| actions.pending_spell_cast.as_ref())
            .expect("current spell transaction survives ordinary Plains mana activation");
        let payment = pending
            .payment_preview
            .as_ref()
            .expect("current spell offer publishes its payment cost");
        let transaction_id = pending.transaction_id;
        let revision = payment.revision;
        assert_eq!(payment.total_cost, expected_total_cost);
        luxior_normal_checkpoint(
            session,
            &format!("before Spell PaymentQuery {card_name}"),
            *accepted_commands,
        );
        let payment_query = session
            .process(&IpcEnvelope {
                msg: Some(Msg::PaymentQuery(PaymentQuery {
                    player_id: 17,
                    preview: Some(PreviewPayment {
                        transaction_id,
                        revision,
                        commit_spell_cast: Some(CommitSpellCast {
                            transaction_id,
                            payment: Some(PaymentSelection {
                                expected_state_revision: revision,
                                mana: Some(produced),
                                ..Default::default()
                            }),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }),
                })),
            })
            .expect("spell payment query keeps the normal session open");
        luxior_normal_checkpoint(
            session,
            &format!(
                "after Spell PaymentQuery {card_name} ok={}",
                payment_query.ok
            ),
            *accepted_commands,
        );
        assert!(
            payment_query.ok,
            "{card_name} payment preview: {}",
            payment_query.error
        );
        let preview = payment_query
            .batch
            .and_then(|batch| batch.payment_preview)
            .expect("PaymentQuery returns the current spell payment preview");
        assert!(
            preview.valid && preview.complete,
            "{card_name} exact white payment is valid: {preview:?}"
        );
        assert_eq!(preview.total_cost, expected_total_cost);
        let selection = preview
            .selection
            .as_ref()
            .expect("complete spell preview returns an exact selection");
        assert_eq!(selection.mana.as_ref(), Some(&produced));
        assert_eq!(produced.w, white_count);
        let player = session
            .engine
            .as_ref()
            .expect("Luxior session has an engine before spell payment")
            .state
            .players
            .iter()
            .find(|player| player.id == 17)
            .expect("pilot remains in the active session");
        assert_eq!(
            player.mana_pool.white, white_count,
            "{card_name} spends only the white mana floated from current Plains"
        );
        let transaction_id = preview.transaction_id;
        latest = luxior_normal_player_command(
            session,
            17,
            RuledCommand {
                cmd: Some(Cmd::CommitSpellCast(CommitSpellCast {
                    transaction_id,
                    payment: preview.selection,
                    restricted_mana: preview.restricted_mana,
                })),
            },
            battlefield,
            handles,
            accepted_commands,
        );
        luxior_normal_assert_white_pool(session, 0, card_name);
        assert!(session
            .engine
            .as_ref()
            .expect("Luxior session has an engine after spell commit")
            .state
            .pending_spell_cast
            .is_none());
        luxior_normal_resolve_stack(session, latest, battlefield, handles, accepted_commands)
    }

    fn luxior_normal_activate_equipment(
        session: &mut EngineSession,
        latest: IpcResponse,
        battlefield: &mut Vec<BattlefieldObject>,
        handles: &mut LuxiorNormalHandles,
        accepted_commands: &mut usize,
        preview_correlation: &mut u64,
        plan: LuxiorNormalEquipmentPlan<'_>,
    ) -> IpcResponse {
        let LuxiorNormalEquipmentPlan {
            stable_ability_id,
            target_object_id,
            white_count,
            expected_total_cost,
        } = plan;
        let (mut latest, produced) = luxior_normal_float_white(
            session,
            latest,
            battlefield,
            handles,
            accepted_commands,
            white_count,
            stable_ability_id,
        );
        let source_handle = handles
            .luxior
            .expect("Luxior public handle was captured from its resolved spell");
        let source = battlefield
            .iter()
            .find(|object| {
                object.object_id == source_handle.0
                    && object.zone_change_generation == source_handle.1
            })
            .expect("latest battlefield retains the current Luxior incarnation");
        let target = battlefield
            .iter()
            .find(|object| object.object_id == target_object_id)
            .expect("latest battlefield retains the offered target incarnation");
        let ability = luxior_normal_current_ability(source, stable_ability_id);
        assert!(
            ability.activatable,
            "current {stable_ability_id} offer is usable"
        );
        assert_eq!(ability.mana_cost, expected_total_cost);
        let target_group =
            luxior_normal_target_group(&latest, source.object_id, ability.ability_index);
        assert_eq!((target_group.min, target_group.max), (1, 1));
        let activation = ActivateAbility {
            source_object_id: source.object_id,
            ability_index: ability.ability_index,
            targets: vec![luxior_normal_target_ref(target_group, target)],
            source_zone: AbilitySourceZone::Battlefield as i32,
            expected_zone_change_generation: source.zone_change_generation,
            ..Default::default()
        };

        // These IDs correlate a stateless preview response; ordinary Equip has no locked
        // engine-owned ability transaction. The returned payment selection carries the exact
        // current state revision used by ActivateAbility.
        *preview_correlation += 1;
        let correlation = *preview_correlation;
        luxior_normal_checkpoint(
            session,
            &format!("before Equip PaymentQuery {stable_ability_id} target={target_object_id} without payment"),
            *accepted_commands,
        );
        let initial_query = session
            .process(&IpcEnvelope {
                msg: Some(Msg::PaymentQuery(PaymentQuery {
                    player_id: 17,
                    preview: Some(PreviewPayment {
                        transaction_id: correlation,
                        revision: correlation,
                        activate_ability: Some(activation.clone()),
                        ..Default::default()
                    }),
                })),
            })
            .expect("initial Equip payment preview keeps the normal session open");
        luxior_normal_checkpoint(
            session,
            &format!(
                "after Equip PaymentQuery {stable_ability_id} target={target_object_id} ok={}",
                initial_query.ok
            ),
            *accepted_commands,
        );
        assert!(
            initial_query.ok,
            "{stable_ability_id} payment preview: {}",
            initial_query.error
        );
        let initial_preview = initial_query
            .batch
            .and_then(|batch| batch.payment_preview)
            .expect("PaymentQuery returns the current stateless Equip cost preview");
        assert!(
            initial_preview.valid,
            "{stable_ability_id} current cost preview is valid: {initial_preview:?}"
        );
        assert_eq!(initial_preview.total_cost, expected_total_cost);
        let mut proposed_payment = initial_preview
            .selection
            .clone()
            .expect("initial Equip preview returns its current state revision");
        let expected_state_revision = proposed_payment.expected_state_revision;
        proposed_payment.mana = Some(produced);
        let mut selected_activation = activation.clone();
        selected_activation.payment = Some(proposed_payment);
        selected_activation.restricted_mana = initial_preview.restricted_mana.clone();
        *preview_correlation += 1;
        let correlation = *preview_correlation;
        luxior_normal_checkpoint(
            session,
            &format!("before Equip PaymentQuery {stable_ability_id} target={target_object_id} with exact payment"),
            *accepted_commands,
        );
        let payment_query = session
            .process(&IpcEnvelope {
                msg: Some(Msg::PaymentQuery(PaymentQuery {
                    player_id: 17,
                    preview: Some(PreviewPayment {
                        transaction_id: correlation,
                        revision: correlation,
                        activate_ability: Some(selected_activation),
                        ..Default::default()
                    }),
                })),
            })
            .expect("selected Equip payment preview keeps the normal session open");
        luxior_normal_checkpoint(
            session,
            &format!(
                "after Equip PaymentQuery {stable_ability_id} target={target_object_id} with exact payment ok={}",
                payment_query.ok
            ),
            *accepted_commands,
        );
        assert!(
            payment_query.ok,
            "{stable_ability_id} selected payment preview: {}",
            payment_query.error
        );
        let preview = payment_query
            .batch
            .and_then(|batch| batch.payment_preview)
            .expect("PaymentQuery returns the selected stateless Equip preview");
        assert!(
            preview.valid && preview.complete,
            "{stable_ability_id} exact white payment is valid: {preview:?}"
        );
        assert_eq!(preview.total_cost, expected_total_cost);
        let selection = preview
            .selection
            .as_ref()
            .expect("complete Equip preview returns a payment selection");
        assert_eq!(selection.expected_state_revision, expected_state_revision);
        assert_eq!(selection.mana.as_ref(), Some(&produced));
        assert_eq!(produced.w, white_count);
        luxior_normal_assert_white_pool(session, white_count, stable_ability_id);

        let stack_len_before = session
            .engine
            .as_ref()
            .expect("Luxior session has an engine before Equip activation")
            .state
            .stack
            .len();
        let mut activation = activation;
        activation.payment = preview.selection;
        activation.restricted_mana = preview.restricted_mana;
        latest = luxior_normal_player_command(
            session,
            17,
            RuledCommand {
                cmd: Some(Cmd::ActivateAbility(activation)),
            },
            battlefield,
            handles,
            accepted_commands,
        );
        let state = &session
            .engine
            .as_ref()
            .expect("Luxior session has an engine after Equip activation")
            .state;
        assert_eq!(state.stack.len(), stack_len_before + 1);
        assert!(state.pending_ability_activation.is_none());
        luxior_normal_assert_white_pool(session, 0, stable_ability_id);
        luxior_normal_resolve_stack(session, latest, battlefield, handles, accepted_commands)
    }

    #[test]
    fn luxior_normal_session_equips_ajani_activates_loyalty_and_reequips_ornithopter() {
        let mut pilot_deck = Vec::with_capacity(60);
        pilot_deck.extend(vec![LUXIOR_NORMAL_NAME.to_string(); 4]);
        pilot_deck.extend(vec![AJANI_NORMAL_NAME.to_string(); 4]);
        pilot_deck.extend(vec![ORNITHOPTER_NORMAL_NAME.to_string(); 4]);
        pilot_deck.extend(vec!["Plains".to_string(); 48]);
        let player_decks = vec![
            PlayerDeck {
                player_id: 17,
                mainboard_card_name: pilot_deck,
                commander_setup: Some(CommanderSetup {
                    declaration: Some(commander_setup::Declaration::NoCommander(true)),
                }),
            },
            PlayerDeck {
                player_id: 29,
                mainboard_card_name: vec!["Plains".to_string(); 60],
                commander_setup: Some(CommanderSetup {
                    declaration: Some(commander_setup::Declaration::NoCommander(true)),
                }),
            },
        ];
        assert_eq!(player_decks[0].mainboard_card_name.len(), 60);
        assert_eq!(player_decks[1].mainboard_card_name.len(), 60);
        let mut session = EngineSession::new(false);
        let validation = session
            .process(&IpcEnvelope {
                msg: Some(Msg::ValidateDeck(ValidateDeck {
                    card_names: vec![
                        LUXIOR_NORMAL_NAME.into(),
                        AJANI_NORMAL_NAME.into(),
                        ORNITHOPTER_NORMAL_NAME.into(),
                        "Plains".into(),
                    ],
                })),
            })
            .expect("ValidateDeck keeps the Luxior session open");
        assert!(
            validation.ok,
            "complete Luxior pilot deck validates: {}",
            validation.error
        );
        assert!(validation.missing_card_names.is_empty());

        let mut latest = session
            .process(&IpcEnvelope {
                msg: Some(Msg::SessionStart(SessionStart {
                    game_id: 7_281_036,
                    seed: 1,
                    player_ids: vec![17, 29],
                    player_decks,
                    dev_commands_enabled: false,
                    diagnostic_capture_enabled: false,
                    ..Default::default()
                })),
            })
            .expect("SessionStart keeps the strict normal Luxior game open");
        assert!(latest.ok, "normal Luxior GameStart: {}", latest.error);
        assert!(!latest.effective_dev_commands_enabled);
        let mut battlefield = Vec::new();
        let mut handles = LuxiorNormalHandles::default();
        kaya_capture_battlefield(&latest, 17, &mut battlefield);
        luxior_normal_refresh_handles(&battlefield, &mut handles);
        let mut accepted_commands = 0usize;
        let starting = kaya_starting_player_offer(&latest)
            .expect("GameStart publishes the current starting-player offer");
        assert!(starting.eligible_starting_player_ids.contains(&17));
        latest = luxior_normal_player_command(
            &mut session,
            starting.deciding_player_id,
            RuledCommand {
                cmd: Some(Cmd::ChooseStartingPlayer(ChooseStartingPlayer {
                    starting_player_id: 17,
                })),
            },
            &mut battlefield,
            &mut handles,
            &mut accepted_commands,
        );
        while let Some(opening) = kaya_mulligan_offer(&latest) {
            latest = luxior_normal_player_command(
                &mut session,
                opening.deciding_player_id,
                RuledCommand {
                    cmd: Some(Cmd::Mulligan(MulliganDecision { keep: true })),
                },
                &mut battlefield,
                &mut handles,
                &mut accepted_commands,
            );
        }
        for _ in 0..4 {
            latest = luxior_normal_pass_priority(
                &mut session,
                latest,
                &mut battlefield,
                &mut handles,
                &mut accepted_commands,
            );
        }
        {
            let state = &session
                .engine
                .as_ref()
                .expect("Luxior session has an engine")
                .state;
            assert_eq!(state.turn_step, tricerules_core::TurnStep::Main1);
            assert_eq!(state.players[state.active_player_idx].id, 17);
            assert_eq!(state.priority_player_id(), 17);
        }

        let mut player17_turns = 0u32;
        let mut last_player17_turn_instance = None;
        let mut preview_correlation = 0u64;
        let mut completed = false;
        for _ in 0..8_192 {
            let (active_player_id, priority_player_id, main_phase, turn_instance) = {
                let state = &session
                    .engine
                    .as_ref()
                    .expect("Luxior session has an engine")
                    .state;
                (
                    state.players[state.active_player_idx].id,
                    state.priority_player_id(),
                    state.turn_step == tricerules_core::TurnStep::Main1,
                    state.turn_instance,
                )
            };
            if active_player_id == 17
                && main_phase
                && last_player17_turn_instance != Some(turn_instance)
            {
                last_player17_turn_instance = Some(turn_instance);
                player17_turns += 1;
                assert!(
                    player17_turns <= 54,
                    "Luxior pilot stops by player 17 turn 54; last public battlefield={:?}",
                    battlefield
                        .iter()
                        .map(|object| (
                            object.card_id.as_str(),
                            object.object_id,
                            object.zone_change_generation
                        ))
                        .collect::<Vec<_>>()
                );
            }

            if active_player_id == 17 && priority_player_id == 17 && main_phase {
                if handles.luxior.is_some()
                    && handles.ajani.is_some()
                    && handles.ornithopter.is_some()
                    && kaya_available_mana_abilities(&battlefield)
                        .iter()
                        .filter(|source| kaya_output_has_color(&source.3, 'W'))
                        .count()
                        >= 4
                {
                    let luxior = luxior_normal_public_permanent(&battlefield, LUXIOR_NORMAL_ID);
                    let ajani = luxior_normal_public_permanent(&battlefield, AJANI_NORMAL_ID);
                    let ornithopter =
                        luxior_normal_public_permanent(&battlefield, ORNITHOPTER_NORMAL_ID);
                    let ajani_object_id = ajani.object_id;
                    let planeswalker_equip = luxior_normal_current_ability(luxior, "activated_01");
                    let ordinary_equip = luxior_normal_current_ability(luxior, "activated_02");
                    assert!(planeswalker_equip.activatable);
                    assert!(ordinary_equip.activatable);
                    assert_eq!(planeswalker_equip.mana_cost, "{1}");
                    assert_eq!(ordinary_equip.mana_cost, "{3}");
                    let planeswalker_targets = luxior_normal_target_group(
                        &latest,
                        luxior.object_id,
                        planeswalker_equip.ability_index,
                    );
                    let ordinary_targets = luxior_normal_target_group(
                        &latest,
                        luxior.object_id,
                        ordinary_equip.ability_index,
                    );
                    assert_eq!((planeswalker_targets.min, planeswalker_targets.max), (1, 1));
                    assert_eq!((ordinary_targets.min, ordinary_targets.max), (1, 1));
                    assert_eq!(
                        planeswalker_targets.valid_permanent_ids,
                        vec![ajani_object_id]
                    );
                    assert_eq!(
                        ordinary_targets.valid_permanent_ids,
                        vec![ornithopter.object_id]
                    );
                    assert!(!planeswalker_targets
                        .valid_permanent_ids
                        .contains(&ornithopter.object_id));
                    assert!(!ordinary_targets
                        .valid_permanent_ids
                        .contains(&ajani_object_id));

                    let combined_turn_instance = session
                        .engine
                        .as_ref()
                        .expect("Luxior session has an engine before combined Equip sequence")
                        .state
                        .turn_instance;
                    let opponent_life_before = session
                        .engine
                        .as_ref()
                        .expect("Luxior session has an engine")
                        .state
                        .players
                        .iter()
                        .find(|player| player.id == 29)
                        .expect("opponent remains in the normal game")
                        .life;
                    let pilot_life_before = session
                        .engine
                        .as_ref()
                        .expect("Luxior session has an engine")
                        .state
                        .players
                        .iter()
                        .find(|player| player.id == 17)
                        .expect("pilot remains in the normal game")
                        .life;

                    luxior_normal_checkpoint(
                        &session,
                        "phase before planeswalker Equip",
                        accepted_commands,
                    );
                    latest = luxior_normal_activate_equipment(
                        &mut session,
                        latest,
                        &mut battlefield,
                        &mut handles,
                        &mut accepted_commands,
                        &mut preview_correlation,
                        LuxiorNormalEquipmentPlan {
                            stable_ability_id: "activated_01",
                            target_object_id: ajani_object_id,
                            white_count: 1,
                            expected_total_cost: "{1}",
                        },
                    );
                    luxior_normal_checkpoint(
                        &session,
                        "phase after planeswalker Equip resolution",
                        accepted_commands,
                    );
                    let luxior = luxior_normal_public_permanent(&battlefield, LUXIOR_NORMAL_ID);
                    let ajani = luxior_normal_public_permanent(&battlefield, AJANI_NORMAL_ID);
                    assert_eq!(
                        luxior
                            .attachment_recipient
                            .as_ref()
                            .and_then(|recipient| recipient.recipient.as_ref()),
                        Some(
                            &tricerules_proto::ruled::v1::attachment_recipient::Recipient::ObjectId(
                                ajani_object_id
                            )
                        )
                    );
                    assert_eq!(ajani.loyalty, 4);
                    assert!(ajani.is_creature);
                    assert!(!ajani.is_planeswalker);
                    assert_eq!((ajani.power, ajani.toughness), (4, 4));
                    {
                        let engine = session
                            .engine
                            .as_ref()
                            .expect("Luxior session has an engine");
                        luxior_normal_checkpoint(
                            &session,
                            "before Ajani creature characteristics",
                            accepted_commands,
                        );
                        let characteristics = engine.characteristics(ajani.object_id).unwrap();
                        luxior_normal_checkpoint(
                            &session,
                            "after Ajani creature characteristics",
                            accepted_commands,
                        );
                        assert!(characteristics.is_creature());
                        assert!(!characteristics.has_type("Planeswalker"));
                        assert!(!characteristics.has_type("Ajani"));
                        assert!(characteristics.is_legendary());
                        assert_eq!(
                            (characteristics.power, characteristics.toughness),
                            (Some(4), Some(4))
                        );
                        assert_eq!(
                            engine.state.objects[&ajani.object_id]
                                .counters
                                .get(&tricerules_cards::primitives::CounterKind::Loyalty)
                                .copied(),
                            Some(4)
                        );
                    }
                    let current_ajani =
                        luxior_normal_public_permanent(&battlefield, AJANI_NORMAL_ID);
                    let loyalty_plus_one =
                        luxior_normal_current_ability(current_ajani, "activated_01");
                    assert!(loyalty_plus_one.activatable);
                    let loyalty_targets = luxior_normal_target_group(
                        &latest,
                        current_ajani.object_id,
                        loyalty_plus_one.ability_index,
                    );
                    assert_eq!((loyalty_targets.min, loyalty_targets.max), (0, 1));
                    luxior_normal_checkpoint(
                        &session,
                        "phase before Ajani loyalty activation",
                        accepted_commands,
                    );
                    latest = luxior_normal_player_command(
                        &mut session,
                        17,
                        RuledCommand {
                            cmd: Some(Cmd::ActivateAbility(ActivateAbility {
                                source_object_id: current_ajani.object_id,
                                ability_index: loyalty_plus_one.ability_index,
                                source_zone: AbilitySourceZone::Battlefield as i32,
                                expected_zone_change_generation: current_ajani
                                    .zone_change_generation,
                                ..Default::default()
                            })),
                        },
                        &mut battlefield,
                        &mut handles,
                        &mut accepted_commands,
                    );
                    luxior_normal_checkpoint(
                        &session,
                        "phase after Ajani loyalty cost",
                        accepted_commands,
                    );
                    let current_ajani =
                        luxior_normal_public_permanent(&battlefield, AJANI_NORMAL_ID);
                    let current_ornithopter =
                        luxior_normal_public_permanent(&battlefield, ORNITHOPTER_NORMAL_ID);
                    assert_eq!(current_ajani.loyalty, 5);
                    assert_eq!((current_ajani.power, current_ajani.toughness), (5, 5));
                    assert_eq!(
                        (current_ornithopter.power, current_ornithopter.toughness),
                        (0, 2)
                    );
                    assert!(current_ornithopter
                        .keywords
                        .iter()
                        .any(|keyword| keyword == "Flying"));
                    assert!(
                        !luxior_normal_current_ability(current_ajani, "activated_01").activatable
                    );
                    {
                        let engine = session
                            .engine
                            .as_ref()
                            .expect("Luxior session has an engine");
                        luxior_normal_checkpoint(
                            &session,
                            "before Ajani post-cost characteristics",
                            accepted_commands,
                        );
                        let characteristics =
                            engine.characteristics(current_ajani.object_id).unwrap();
                        luxior_normal_checkpoint(
                            &session,
                            "after Ajani post-cost characteristics",
                            accepted_commands,
                        );
                        assert!(characteristics.is_creature());
                        assert_eq!(
                            (characteristics.power, characteristics.toughness),
                            (Some(5), Some(5))
                        );
                        assert_eq!(
                            engine.state.objects[&current_ajani.object_id]
                                .counters
                                .get(&tricerules_cards::primitives::CounterKind::Loyalty)
                                .copied(),
                            Some(5)
                        );
                        assert!(!engine.state.stack.is_empty());
                    }
                    latest = luxior_normal_resolve_stack(
                        &mut session,
                        latest,
                        &mut battlefield,
                        &mut handles,
                        &mut accepted_commands,
                    );
                    luxior_normal_checkpoint(
                        &session,
                        "phase after Ajani loyalty resolution",
                        accepted_commands,
                    );
                    assert!(session
                        .engine
                        .as_ref()
                        .expect("Luxior session has an engine after Ajani resolves")
                        .state
                        .stack
                        .is_empty());
                    let current_luxior =
                        luxior_normal_public_permanent(&battlefield, LUXIOR_NORMAL_ID);
                    let current_ajani =
                        luxior_normal_public_permanent(&battlefield, AJANI_NORMAL_ID);
                    let current_ornithopter =
                        luxior_normal_public_permanent(&battlefield, ORNITHOPTER_NORMAL_ID);
                    let creature_targets = luxior_normal_target_group(
                        &latest,
                        current_luxior.object_id,
                        luxior_normal_current_ability(current_luxior, "activated_02").ability_index,
                    );
                    let planeswalker_targets = luxior_normal_target_group(
                        &latest,
                        current_luxior.object_id,
                        luxior_normal_current_ability(current_luxior, "activated_01").ability_index,
                    );
                    assert_eq!((creature_targets.min, creature_targets.max), (1, 1));
                    assert!(creature_targets
                        .valid_permanent_ids
                        .contains(&current_ajani.object_id));
                    let ornithopter_object_id = current_ornithopter.object_id;
                    assert!(creature_targets
                        .valid_permanent_ids
                        .contains(&ornithopter_object_id));
                    assert!(!creature_targets
                        .valid_permanent_ids
                        .contains(&current_luxior.object_id));
                    assert!(!planeswalker_targets
                        .valid_permanent_ids
                        .contains(&current_ajani.object_id));
                    assert!(!planeswalker_targets
                        .valid_permanent_ids
                        .contains(&ornithopter_object_id));

                    luxior_normal_checkpoint(
                        &session,
                        "phase before ordinary Equip",
                        accepted_commands,
                    );
                    latest = luxior_normal_activate_equipment(
                        &mut session,
                        latest,
                        &mut battlefield,
                        &mut handles,
                        &mut accepted_commands,
                        &mut preview_correlation,
                        LuxiorNormalEquipmentPlan {
                            stable_ability_id: "activated_02",
                            target_object_id: ornithopter_object_id,
                            white_count: 3,
                            expected_total_cost: "{3}",
                        },
                    );
                    luxior_normal_checkpoint(
                        &session,
                        "phase after ordinary Equip resolution",
                        accepted_commands,
                    );
                    assert_eq!(
                        session
                            .engine
                            .as_ref()
                            .expect("Luxior session has an engine after re-equip")
                            .state
                            .turn_instance,
                        combined_turn_instance,
                        "both Equip resolutions and Ajani loyalty activation occur in the same turn"
                    );
                    assert_eq!(
                        session
                            .engine
                            .as_ref()
                            .expect("Luxior session has an engine after re-equip")
                            .state
                            .turn_step,
                        tricerules_core::TurnStep::Main1
                    );
                    let current_luxior =
                        luxior_normal_public_permanent(&battlefield, LUXIOR_NORMAL_ID);
                    let current_ajani =
                        luxior_normal_public_permanent(&battlefield, AJANI_NORMAL_ID);
                    let current_ornithopter =
                        luxior_normal_public_permanent(&battlefield, ORNITHOPTER_NORMAL_ID);
                    assert_eq!(
                        current_luxior
                            .attachment_recipient
                            .as_ref()
                            .and_then(|recipient| recipient.recipient.as_ref()),
                        Some(
                            &tricerules_proto::ruled::v1::attachment_recipient::Recipient::ObjectId(
                                current_ornithopter.object_id
                            )
                        )
                    );
                    assert_eq!(current_ajani.loyalty, 5);
                    assert!(!current_ajani.is_creature);
                    assert!(current_ajani.is_planeswalker);
                    assert_eq!(current_ornithopter.loyalty, 0);
                    assert!(current_ornithopter.is_creature);
                    assert_eq!(
                        (current_ornithopter.power, current_ornithopter.toughness),
                        (0, 2)
                    );
                    assert!(current_ornithopter
                        .keywords
                        .iter()
                        .any(|keyword| keyword == "Flying"));
                    {
                        let engine = session
                            .engine
                            .as_ref()
                            .expect("Luxior session has an engine");
                        luxior_normal_checkpoint(
                            &session,
                            "before final Ajani characteristics",
                            accepted_commands,
                        );
                        let ajani_characteristics =
                            engine.characteristics(current_ajani.object_id).unwrap();
                        luxior_normal_checkpoint(
                            &session,
                            "after final Ajani characteristics",
                            accepted_commands,
                        );
                        assert!(!ajani_characteristics.is_creature());
                        assert!(ajani_characteristics.has_type("Planeswalker"));
                        assert!(ajani_characteristics.has_type("Ajani"));
                        assert!(ajani_characteristics.is_legendary());
                        assert_eq!(
                            (ajani_characteristics.power, ajani_characteristics.toughness),
                            (None, None)
                        );
                        assert_eq!(
                            engine.state.objects[&current_ajani.object_id]
                                .counters
                                .get(&tricerules_cards::primitives::CounterKind::Loyalty)
                                .copied(),
                            Some(5)
                        );
                        luxior_normal_checkpoint(
                            &session,
                            "before final Ornithopter characteristics",
                            accepted_commands,
                        );
                        let ornithopter_characteristics = engine
                            .characteristics(current_ornithopter.object_id)
                            .unwrap();
                        luxior_normal_checkpoint(
                            &session,
                            "after final Ornithopter characteristics",
                            accepted_commands,
                        );
                        assert!(ornithopter_characteristics.is_artifact());
                        assert!(ornithopter_characteristics.is_creature());
                        assert!(ornithopter_characteristics.has_type("Thopter"));
                        assert!(ornithopter_characteristics
                            .has_keyword(tricerules_cards::primitives::Keyword::Flying));
                        assert_eq!(
                            (
                                ornithopter_characteristics.power,
                                ornithopter_characteristics.toughness
                            ),
                            (Some(0), Some(2))
                        );
                        assert!(engine.state.objects[&current_ornithopter.object_id]
                            .counters
                            .is_empty());
                        assert!(engine.state.stack.is_empty());
                        assert!(engine.state.pending_spell_cast.is_none());
                        assert!(engine.state.pending_ability_activation.is_none());
                        let player_idx = engine.state.player_idx(17).unwrap();
                        for handle in [
                            handles.luxior.expect("Luxior handle remains captured"),
                            handles.ajani.expect("Ajani handle remains captured"),
                            handles
                                .ornithopter
                                .expect("Ornithopter handle remains captured"),
                        ] {
                            assert!(engine.state.players[player_idx]
                                .battlefield
                                .contains(&handle.0));
                        }
                        assert_eq!(
                            engine.state.players[player_idx]
                                .battlefield
                                .iter()
                                .filter_map(|object_id| engine.state.objects.get(object_id))
                                .filter(|object| object.card_id != "plains")
                                .map(|object| object.card_id.as_str())
                                .collect::<std::collections::BTreeSet<_>>(),
                            [LUXIOR_NORMAL_ID, AJANI_NORMAL_ID, ORNITHOPTER_NORMAL_ID]
                                .into_iter()
                                .collect()
                        );
                        assert_eq!(
                            engine
                                .state
                                .players
                                .iter()
                                .find(|player| player.id == 17)
                                .unwrap()
                                .life,
                            pilot_life_before
                        );
                        assert_eq!(
                            engine
                                .state
                                .players
                                .iter()
                                .find(|player| player.id == 29)
                                .unwrap()
                                .life,
                            opponent_life_before
                        );
                    }
                    let final_planeswalker_targets = luxior_normal_target_group(
                        &latest,
                        current_luxior.object_id,
                        luxior_normal_current_ability(current_luxior, "activated_01").ability_index,
                    );
                    let final_creature_targets = luxior_normal_target_group(
                        &latest,
                        current_luxior.object_id,
                        luxior_normal_current_ability(current_luxior, "activated_02").ability_index,
                    );
                    assert!(final_planeswalker_targets
                        .valid_permanent_ids
                        .contains(&current_ajani.object_id));
                    assert!(!final_creature_targets
                        .valid_permanent_ids
                        .contains(&current_ajani.object_id));
                    assert_eq!(
                        final_creature_targets.valid_permanent_ids,
                        vec![current_ornithopter.object_id]
                    );
                    assert!(
                        !luxior_normal_current_ability(current_ajani, "activated_01").activatable
                    );
                    completed = true;
                    break;
                }

                let actions = kaya_batch(&latest)
                    .legal_by_player
                    .get(&17)
                    .expect("pilot receives current main-phase actions")
                    .clone();
                let pilot_plains_on_battlefield = battlefield
                    .iter()
                    .filter(|object| object.card_id == "plains")
                    .count();
                if pilot_plains_on_battlefield < 4 {
                    if let Some(land) = actions
                        .hand_actions
                        .iter()
                        .find(|offer| offer.kind == HandActionKind::HandActionPlayLand as i32)
                        .cloned()
                    {
                        latest = luxior_normal_player_command(
                            &mut session,
                            17,
                            RuledCommand {
                                cmd: Some(Cmd::PlayLand(PlayLand {
                                    source: Some(LandSource {
                                        location: Some(
                                            tricerules_proto::ruled::v1::land_source::Location::
                                                HandIndex(land.hand_index),
                                        ),
                                        ..Default::default()
                                    }),
                                    face_index: land.face_index,
                                    ..Default::default()
                                })),
                            },
                            &mut battlefield,
                            &mut handles,
                            &mut accepted_commands,
                        );
                        continue;
                    }
                }

                let castable = [
                    (ORNITHOPTER_NORMAL_NAME, handles.ornithopter, "{0}", 0),
                    (LUXIOR_NORMAL_NAME, handles.luxior, "{1}", 1),
                    (AJANI_NORMAL_NAME, handles.ajani, "{3}{W}", 4),
                ];
                let white_available = kaya_available_mana_abilities(&battlefield)
                    .iter()
                    .filter(|source| kaya_output_has_color(&source.3, 'W'))
                    .count();
                if let Some((card_name, None, expected_cost, white_count)) =
                    castable.into_iter().find(|(card_name, handle, _, cost)| {
                        handle.is_none()
                            && *cost as usize <= white_available
                            && actions.hand_actions.iter().any(|offer| {
                                offer.kind == HandActionKind::HandActionCastSpell as i32
                                    && offer.card_name == *card_name
                            })
                    })
                {
                    latest = luxior_normal_cast_from_current_offer(
                        &mut session,
                        latest,
                        &mut battlefield,
                        &mut handles,
                        &mut accepted_commands,
                        LuxiorNormalCastPlan {
                            card_name,
                            expected_total_cost: expected_cost,
                            white_count,
                        },
                    );
                    continue;
                }
            }

            latest = luxior_normal_pass_priority(
                &mut session,
                latest,
                &mut battlefield,
                &mut handles,
                &mut accepted_commands,
            );
        }
        let state = &session
            .engine
            .as_ref()
            .expect("Luxior session has an engine after bounded progression")
            .state;
        assert!(
            completed,
            "normal Luxior pilot did not complete within 8192 accepted commands / 54 player-17 turns; commands={accepted_commands}, turns={player17_turns}, active={}, priority={}, step={:?}, battlefield={:?}",
            state.players[state.active_player_idx].id,
            state.priority_player_id(),
            state.turn_step,
            battlefield
                .iter()
                .map(|object| (object.card_id.as_str(), object.object_id, object.zone_change_generation))
                .collect::<Vec<_>>()
        );
        assert!(accepted_commands <= 8_192);
        assert_eq!(
            player17_turns,
            last_player17_turn_instance.map_or(0, |_| player17_turns)
        );
    }

    #[test]
    fn ugin_normal_session_casts_from_current_offers_and_activates_plus_two() {
        const UGIN_NAME: &str = "Ugin, the Spirit Dragon";
        const UGIN_CARD_ID: &str = "ugin,_the_spirit_dragon";

        let make_deck = |player_id: i32, ugin_pilot: bool| {
            let mut mainboard_card_name = if ugin_pilot {
                vec![UGIN_NAME.to_string(); 4]
            } else {
                Vec::new()
            };
            mainboard_card_name.extend(vec!["Forest".to_string(); 60 - mainboard_card_name.len()]);
            PlayerDeck {
                player_id,
                mainboard_card_name,
                commander_setup: Some(CommanderSetup {
                    declaration: Some(commander_setup::Declaration::NoCommander(true)),
                }),
            }
        };
        let player_decks = vec![make_deck(17, true), make_deck(29, false)];
        assert_eq!(player_decks[0].mainboard_card_name.len(), 60);
        assert_eq!(player_decks[1].mainboard_card_name.len(), 60);

        let mut session = EngineSession::new(false);
        let validation = session
            .process(&IpcEnvelope {
                msg: Some(Msg::ValidateDeck(ValidateDeck {
                    card_names: vec![UGIN_NAME.into(), "Forest".into()],
                })),
            })
            .expect("deck validation leaves the session open");
        assert!(
            validation.ok,
            "complete Ugin decks validate: {}",
            validation.error
        );
        assert!(validation.missing_card_names.is_empty());

        let mut latest = session
            .process(&IpcEnvelope {
                msg: Some(Msg::SessionStart(SessionStart {
                    game_id: 7_281_031,
                    seed: 20_261_029,
                    player_ids: vec![17, 29],
                    player_decks,
                    dev_commands_enabled: false,
                    diagnostic_capture_enabled: false,
                    ..Default::default()
                })),
            })
            .expect("SessionStart keeps the normal game session open");
        assert!(latest.ok, "normal Ugin GameStart: {}", latest.error);
        assert!(!latest.effective_dev_commands_enabled);

        let starting = kaya_starting_player_offer(&latest)
            .expect("GameStart publishes the eligible starting-player offer");
        assert!(starting.eligible_starting_player_ids.contains(&17));
        latest = kaya_player_command(
            &mut session,
            starting.deciding_player_id,
            RuledCommand {
                cmd: Some(Cmd::ChooseStartingPlayer(ChooseStartingPlayer {
                    starting_player_id: 17,
                })),
            },
        );
        while let Some(opening) = kaya_mulligan_offer(&latest) {
            latest = kaya_player_command(
                &mut session,
                opening.deciding_player_id,
                RuledCommand {
                    cmd: Some(Cmd::Mulligan(MulliganDecision { keep: true })),
                },
            );
        }
        for _ in 0..4 {
            latest = ugin_pass_priority(&mut session, latest);
        }
        {
            let state = &session.engine.as_ref().unwrap().state;
            assert_eq!(state.turn_step, tricerules_core::TurnStep::Main1);
            assert_eq!(state.players[state.active_player_idx].id, 17);
        }

        let mut battlefield = Vec::new();
        kaya_capture_battlefield(&latest, 17, &mut battlefield);
        let ugin_from_view = |objects: &[BattlefieldObject]| {
            objects
                .iter()
                .find(|object| object.card_id == UGIN_CARD_ID)
                .cloned()
        };
        let mut ugin_cast = false;
        for _ in 0..4_096 {
            let (active_player_id, priority_player_id, main_phase) = {
                let state = &session.engine.as_ref().unwrap().state;
                (
                    state.players[state.active_player_idx].id,
                    state.priority_player_id(),
                    state.turn_step == tricerules_core::TurnStep::Main1,
                )
            };

            if active_player_id == 17 && priority_player_id == 17 && main_phase {
                let actions = kaya_batch(&latest)
                    .legal_by_player
                    .get(&17)
                    .expect("active pilot receives current legal actions")
                    .clone();
                let land = actions
                    .hand_actions
                    .iter()
                    .find(|action| action.kind == HandActionKind::HandActionPlayLand as i32)
                    .cloned();
                if let Some(land) = land {
                    latest = kaya_player_command(
                        &mut session,
                        17,
                        RuledCommand {
                            cmd: Some(Cmd::PlayLand(PlayLand {
                                source: Some(LandSource {
                                    location: Some(
                                        tricerules_proto::ruled::v1::land_source::Location::
                                            HandIndex(land.hand_index),
                                    ),
                                    ..Default::default()
                                }),
                                face_index: land.face_index,
                                ..Default::default()
                            })),
                        },
                    );
                    kaya_capture_battlefield(&latest, 17, &mut battlefield);
                    continue;
                }

                let ugin_spell = actions
                    .hand_actions
                    .iter()
                    .find(|action| {
                        action.kind == HandActionKind::HandActionCastSpell as i32
                            && action.card_name == UGIN_NAME
                    })
                    .cloned();
                let mana_sources = kaya_available_mana_abilities(&battlefield)
                    .into_iter()
                    .filter(|source| kaya_output_has_color(&source.3, 'G'))
                    .collect::<Vec<_>>();
                if let Some(spell) = ugin_spell.filter(|_| mana_sources.len() >= 8) {
                    latest = kaya_player_command(
                        &mut session,
                        17,
                        RuledCommand {
                            cmd: Some(Cmd::BeginSpellCast(BeginSpellCast {
                                announcement: Some(
                                    tricerules_proto::ruled::v1::SpellCastAnnouncement {
                                        source: Some(CastSource {
                                            location: Some(
                                                tricerules_proto::ruled::v1::cast_source::Location::
                                                    HandIndex(spell.hand_index),
                                            ),
                                            expected_zone_change_generation: None,
                                        }),
                                        face_index: spell.face_index,
                                        cast_method: CastMethod::Normal as i32,
                                        ..Default::default()
                                    },
                                ),
                            })),
                        },
                    );

                    let mut produced = PaymentMana::default();
                    for (source_object_id, generation, ability_index, output) in
                        mana_sources.into_iter().take(8)
                    {
                        let current_source = kaya_available_mana_abilities(&battlefield)
                            .into_iter()
                            .find(|source| {
                                source.0 == source_object_id
                                    && source.1 == generation
                                    && source.2 == ability_index
                            })
                            .expect("current land view retains this untapped mana offer");
                        assert_eq!(current_source.3, output);
                        assert!(kaya_output_has_color(&output, 'G'));
                        produced.g += 1;
                        latest = kaya_player_command(
                            &mut session,
                            17,
                            RuledCommand {
                                cmd: Some(Cmd::ActivateAbility(ActivateAbility {
                                    source_object_id,
                                    ability_index,
                                    source_zone: AbilitySourceZone::Battlefield as i32,
                                    expected_zone_change_generation: generation,
                                    ..Default::default()
                                })),
                            },
                        );
                        kaya_capture_battlefield(&latest, 17, &mut battlefield);
                    }

                    let pending = kaya_batch(&latest)
                        .legal_by_player
                        .get(&17)
                        .and_then(|actions| actions.pending_spell_cast.as_ref())
                        .expect("current Ugin payment offer survives mana activation");
                    let payment = pending
                        .payment_preview
                        .as_ref()
                        .expect("pending Ugin cast publishes its payment preview");
                    let transaction_id = pending.transaction_id;
                    let revision = payment.revision;
                    let payment_query = session
                        .process(&IpcEnvelope {
                            msg: Some(Msg::PaymentQuery(PaymentQuery {
                                player_id: 17,
                                preview: Some(PreviewPayment {
                                    transaction_id,
                                    revision,
                                    commit_spell_cast: Some(CommitSpellCast {
                                        transaction_id,
                                        payment: Some(PaymentSelection {
                                            expected_state_revision: revision,
                                            mana: Some(produced),
                                            ..Default::default()
                                        }),
                                        ..Default::default()
                                    }),
                                    ..Default::default()
                                }),
                            })),
                        })
                        .expect("Ugin payment query keeps the session open");
                    assert!(
                        payment_query.ok,
                        "Ugin payment preview: {}",
                        payment_query.error
                    );
                    let preview = payment_query
                        .batch
                        .and_then(|batch| batch.payment_preview)
                        .expect("PaymentQuery returns a current Ugin payment preview");
                    assert!(
                        preview.valid && preview.complete,
                        "Ugin payment: {preview:?}"
                    );
                    latest = kaya_player_command(
                        &mut session,
                        17,
                        RuledCommand {
                            cmd: Some(Cmd::CommitSpellCast(CommitSpellCast {
                                transaction_id,
                                payment: preview.selection,
                                restricted_mana: preview.restricted_mana,
                            })),
                        },
                    );
                    kaya_capture_battlefield(&latest, 17, &mut battlefield);
                    for _ in 0..8 {
                        if ugin_from_view(&battlefield).is_some() {
                            ugin_cast = true;
                            break;
                        }
                        latest = ugin_pass_priority(&mut session, latest);
                        kaya_capture_battlefield(&latest, 17, &mut battlefield);
                    }
                    assert!(ugin_cast, "Ugin resolves through ordinary priority passes");
                    break;
                }
            }

            latest = ugin_pass_priority(&mut session, latest);
            kaya_capture_battlefield(&latest, 17, &mut battlefield);
        }
        assert!(ugin_cast, "ordinary offers eventually cast Ugin");

        let ugin = ugin_from_view(&battlefield).expect("current view publishes Ugin");
        let plus_two = ugin
            .activated_abilities
            .iter()
            .find(|ability| {
                ability.presentation.as_ref().is_some_and(|presentation| {
                    presentation.path.iter().any(|component| {
                        component.kind == PresentationPathKind::Ability as i32
                            && component.id == "activated_01"
                    })
                })
            })
            .expect("current Ugin view identifies +2 by stable ability ID");
        assert!(plus_two.activatable, "current +2 ability offer is legal");
        let plus_two_key = (u64::from(ugin.object_id) << 32) | u64::from(plus_two.ability_index);
        let legal_actions = kaya_batch(&latest)
            .legal_by_player
            .get(&17)
            .expect("the Ugin controller receives current legal actions");
        let target_group = legal_actions
            .valid_targets_by_ability
            .get(&plus_two_key)
            .and_then(|targets| targets.groups.first())
            .expect("the current stable +2 offer publishes its target group");
        assert_eq!((target_group.min, target_group.max), (1, 1));
        assert!(
            target_group.can_target_opponent,
            "the current +2 offer includes the active opponent as a legal target"
        );

        let opponent_life_before = session
            .engine
            .as_ref()
            .unwrap()
            .state
            .players
            .iter()
            .find(|player| player.id == 29)
            .expect("opponent is in the normal game session")
            .life;
        latest = kaya_player_command(
            &mut session,
            17,
            RuledCommand {
                cmd: Some(Cmd::ActivateAbility(ActivateAbility {
                    source_object_id: ugin.object_id,
                    ability_index: plus_two.ability_index,
                    targets: vec![TargetRef {
                        object_id: 29,
                        group_index: 0,
                        kind: TargetRefKind::Player as i32,
                        ..Default::default()
                    }],
                    source_zone: AbilitySourceZone::Battlefield as i32,
                    expected_zone_change_generation: ugin.zone_change_generation,
                    ..Default::default()
                })),
            },
        );
        kaya_capture_battlefield(&latest, 17, &mut battlefield);
        let current_ugin = ugin_from_view(&battlefield).expect("current view retains Ugin");
        assert_eq!(current_ugin.loyalty, ugin.loyalty + 2);
        for _ in 0..4 {
            latest = ugin_pass_priority(&mut session, latest);
            kaya_capture_battlefield(&latest, 17, &mut battlefield);
            let opponent_life = session
                .engine
                .as_ref()
                .unwrap()
                .state
                .players
                .iter()
                .find(|player| player.id == 29)
                .expect("opponent is in the normal game session")
                .life;
            if opponent_life == opponent_life_before - 3 {
                break;
            }
        }
        let opponent_life = session
            .engine
            .as_ref()
            .unwrap()
            .state
            .players
            .iter()
            .find(|player| player.id == 29)
            .expect("opponent is in the normal game session")
            .life;
        assert_eq!(opponent_life, opponent_life_before - 3);
        assert!(session.engine.as_ref().unwrap().state.stack.is_empty());
    }

    #[test]
    fn kaya_zero_legal_session_casts_from_offers_and_reaches_the_optional_scry_leaf() {
        let player_decks = vec![kaya_player_deck(17, true), kaya_player_deck(29, false)];
        assert_eq!(player_decks[0].mainboard_card_name.len(), 60);
        assert_eq!(player_decks[1].mainboard_card_name.len(), 60);

        let mut session = EngineSession::new(false);
        let validation = session
            .process(&IpcEnvelope {
                msg: Some(Msg::ValidateDeck(ValidateDeck {
                    card_names: vec![
                        "Kaya, Intangible Slayer".into(),
                        "Plains".into(),
                        "Swamp".into(),
                    ],
                })),
            })
            .expect("deck validation leaves the session open");
        assert!(
            validation.ok,
            "complete Kaya deck validates: {}",
            validation.error
        );
        assert!(validation.missing_card_names.is_empty());

        let mut latest = session
            .process(&IpcEnvelope {
                msg: Some(Msg::SessionStart(SessionStart {
                    game_id: 7_281_028,
                    seed: 20_261_028,
                    player_ids: vec![17, 29],
                    player_decks,
                    dev_commands_enabled: false,
                    diagnostic_capture_enabled: false,
                    ..Default::default()
                })),
            })
            .expect("SessionStart keeps the normal game session open");
        assert!(latest.ok, "normal Kaya GameStart: {}", latest.error);
        assert!(!latest.effective_dev_commands_enabled);

        let starting = kaya_starting_player_offer(&latest)
            .expect("GameStart publishes the eligible starting-player offer");
        assert!(starting.eligible_starting_player_ids.contains(&17));
        latest = kaya_player_command(
            &mut session,
            starting.deciding_player_id,
            RuledCommand {
                cmd: Some(Cmd::ChooseStartingPlayer(ChooseStartingPlayer {
                    starting_player_id: if starting.eligible_starting_player_ids.contains(&17) {
                        17
                    } else {
                        starting.eligible_starting_player_ids[0]
                    },
                })),
            },
        );
        while let Some(opening) = kaya_mulligan_offer(&latest) {
            latest = kaya_player_command(
                &mut session,
                opening.deciding_player_id,
                RuledCommand {
                    cmd: Some(Cmd::Mulligan(MulliganDecision { keep: true })),
                },
            );
        }

        for _ in 0..4 {
            latest = kaya_pass_priority(&mut session, latest);
        }
        {
            let state = &session.engine.as_ref().unwrap().state;
            assert_eq!(state.turn_step, tricerules_core::TurnStep::Main1);
            assert_eq!(state.players[state.active_player_idx].id, 17);
        }

        let mut battlefield = Vec::new();
        kaya_capture_battlefield(&latest, 17, &mut battlefield);
        let mut committed = false;
        for _ in 0..2_000 {
            let (active_player_id, priority_player_id, main_phase) = {
                let engine = session.engine.as_ref().unwrap();
                let state = &engine.state;
                (
                    state.players[state.active_player_idx].id,
                    state.priority_player_id(),
                    state.turn_step == tricerules_core::TurnStep::Main1,
                )
            };

            if active_player_id == 17 && priority_player_id == 17 && main_phase {
                let pilot_actions = kaya_batch(&latest)
                    .legal_by_player
                    .get(&17)
                    .expect("active pilot receives current legal actions")
                    .clone();
                if let Some(land) = pilot_actions
                    .hand_actions
                    .iter()
                    .find(|action| action.kind == HandActionKind::HandActionPlayLand as i32)
                    .cloned()
                {
                    latest = kaya_player_command(
                        &mut session,
                        17,
                        RuledCommand {
                            cmd: Some(Cmd::PlayLand(PlayLand {
                                source: Some(LandSource {
                                    location: Some(
                                        tricerules_proto::ruled::v1::land_source::Location::
                                            HandIndex(land.hand_index),
                                    ),
                                    ..Default::default()
                                }),
                                face_index: land.face_index,
                                ..Default::default()
                            })),
                        },
                    );
                    kaya_capture_battlefield(&latest, 17, &mut battlefield);
                    continue;
                }

                let kaya_spell = pilot_actions
                    .hand_actions
                    .iter()
                    .find(|action| {
                        action.kind == HandActionKind::HandActionCastSpell as i32
                            && action.card_name == "Kaya, Intangible Slayer"
                    })
                    .cloned();
                if let Some(spell) =
                    kaya_spell.filter(|_| kaya_can_pay_from_offered_lands(&battlefield))
                {
                    latest = kaya_player_command(
                        &mut session,
                        17,
                        RuledCommand {
                            cmd: Some(Cmd::BeginSpellCast(BeginSpellCast {
                                announcement: Some(
                                    tricerules_proto::ruled::v1::SpellCastAnnouncement {
                                        source: Some(CastSource {
                                            location: Some(
                                                tricerules_proto::ruled::v1::cast_source::Location::
                                                    HandIndex(spell.hand_index),
                                            ),
                                            expected_zone_change_generation: None,
                                        }),
                                        face_index: spell.face_index,
                                        cast_method: CastMethod::Normal as i32,
                                        ..Default::default()
                                    },
                                ),
                            })),
                        },
                    );
                    kaya_capture_battlefield(&latest, 17, &mut battlefield);

                    let mut produced = PaymentMana::default();
                    for _ in 0..7 {
                        let mana_abilities = kaya_available_mana_abilities(&battlefield);
                        let preferred = if produced.w < 2 {
                            Some('W')
                        } else if produced.b < 2 {
                            Some('B')
                        } else {
                            None
                        };
                        let (source_object_id, generation, ability_index, output) =
                            mana_abilities
                                .iter()
                                .find(|ability| {
                                    preferred.is_none_or(|color| {
                                        kaya_output_has_color(&ability.3, color)
                                    })
                                })
                                .cloned()
                                .expect("current battlefield view offers seven suitable basic-land mana abilities");
                        let has_white = kaya_output_has_color(&output, 'W');
                        let has_black = kaya_output_has_color(&output, 'B');
                        assert_ne!(
                            has_white, has_black,
                            "each offered basic land makes one color"
                        );
                        if has_white {
                            produced.w += 1;
                        } else {
                            produced.b += 1;
                        }
                        latest = kaya_player_command(
                            &mut session,
                            17,
                            RuledCommand {
                                cmd: Some(Cmd::ActivateAbility(ActivateAbility {
                                    source_object_id,
                                    ability_index,
                                    source_zone: AbilitySourceZone::Battlefield as i32,
                                    expected_zone_change_generation: generation,
                                    ..Default::default()
                                })),
                            },
                        );
                        kaya_capture_battlefield(&latest, 17, &mut battlefield);
                    }
                    assert_eq!(produced.w + produced.b, 7);
                    assert!(produced.w >= 2 && produced.b >= 2);

                    let pending = kaya_batch(&latest)
                        .legal_by_player
                        .get(&17)
                        .and_then(|actions| actions.pending_spell_cast.as_ref())
                        .expect("current cast-payment offer survives mana activation");
                    let pending_payment = pending
                        .payment_preview
                        .as_ref()
                        .expect("the pending cast publishes payment preview");
                    let transaction_id = pending.transaction_id;
                    let revision = pending_payment.revision;
                    let proposed_payment = PaymentSelection {
                        expected_state_revision: revision,
                        mana: Some(produced),
                        ..Default::default()
                    };
                    let payment_query = session
                        .process(&IpcEnvelope {
                            msg: Some(Msg::PaymentQuery(PaymentQuery {
                                player_id: 17,
                                preview: Some(PreviewPayment {
                                    transaction_id,
                                    revision,
                                    commit_spell_cast: Some(CommitSpellCast {
                                        transaction_id,
                                        payment: Some(proposed_payment),
                                        ..Default::default()
                                    }),
                                    ..Default::default()
                                }),
                            })),
                        })
                        .expect("payment query keeps the session open");
                    assert!(
                        payment_query.ok,
                        "Kaya payment preview: {}",
                        payment_query.error
                    );
                    let preview = payment_query
                        .batch
                        .and_then(|batch| batch.payment_preview)
                        .expect("PaymentQuery returns a current payment preview");
                    assert!(
                        preview.valid && preview.complete,
                        "Kaya payment: {preview:?}"
                    );
                    latest = kaya_player_command(
                        &mut session,
                        17,
                        RuledCommand {
                            cmd: Some(Cmd::CommitSpellCast(CommitSpellCast {
                                transaction_id,
                                payment: preview.selection,
                                restricted_mana: preview.restricted_mana,
                            })),
                        },
                    );
                    kaya_capture_battlefield(&latest, 17, &mut battlefield);

                    for _ in 0..8 {
                        if kaya_on_battlefield(&session).is_some() {
                            committed = true;
                            break;
                        }
                        latest = kaya_pass_priority(&mut session, latest);
                        kaya_capture_battlefield(&latest, 17, &mut battlefield);
                    }
                    assert!(
                        committed,
                        "Kaya spell resolves through ordinary priority passes"
                    );
                    break;
                }
            }

            latest = kaya_pass_priority(&mut session, latest);
            kaya_capture_battlefield(&latest, 17, &mut battlefield);
        }
        assert!(
            committed,
            "60-card normal session finds and casts Kaya using offered lands"
        );

        let (source_object_id, generation) =
            kaya_on_battlefield(&session).expect("normal Kaya spell is on the battlefield");
        let source_view = battlefield
            .iter()
            .find(|permanent| permanent.object_id == source_object_id)
            .expect("current server response publishes Kaya's battlefield view");
        assert_eq!(source_view.zone_change_generation, generation);
        let zero_ability = source_view
            .activated_abilities
            .iter()
            .find(|ability| {
                ability.presentation.as_ref().is_some_and(|presentation| {
                    presentation.path.iter().any(|component| {
                        component.kind == PresentationPathKind::Ability as i32
                            && component.id == "activated_02"
                    })
                })
            })
            .expect("the published Kaya view identifies the zero-loyalty ability by stable ID");
        assert!(zero_ability.activatable);
        let hand_before = session.engine.as_ref().unwrap().state.players[0].hand.len();
        let library_before = session.engine.as_ref().unwrap().state.players[0]
            .library
            .len();
        latest = kaya_player_command(
            &mut session,
            17,
            RuledCommand {
                cmd: Some(Cmd::ActivateAbility(ActivateAbility {
                    source_object_id,
                    ability_index: zero_ability.ability_index,
                    source_zone: AbilitySourceZone::Battlefield as i32,
                    expected_zone_change_generation: generation,
                    ..Default::default()
                })),
            },
        );
        kaya_capture_battlefield(&latest, 17, &mut battlefield);

        latest = kaya_pass_priority(&mut session, latest);
        assert!(
            kaya_choice(&latest, ChoiceKind::ResolutionBranch).is_none(),
            "first priority pass does not run Kaya's ability"
        );
        latest = kaya_pass_priority(&mut session, latest);
        let opt_in = kaya_choice(&latest, ChoiceKind::ResolutionBranch)
            .expect("the first opponent receives the current Scry 1 opt-in");
        assert_eq!(opt_in.deciding_player_id, 29);
        assert_ne!(opt_in.resolution_prompt_id, 0);
        assert_eq!(
            session.engine.as_ref().unwrap().state.players[0].hand.len(),
            hand_before + 2
        );
        assert_eq!(
            session.engine.as_ref().unwrap().state.players[0]
                .library
                .len(),
            library_before - 2,
            "Kaya draws both cards before its first opponent opt-in"
        );
        let accept_branch = opt_in
            .resolution_branches
            .iter()
            .find(|branch| branch.branch_index == 0 && branch.selectable)
            .expect("the current offer includes Scry 1");
        latest = kaya_player_command(
            &mut session,
            29,
            RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    resolution_prompt_id: opt_in.resolution_prompt_id,
                    decision: ResolutionChoiceDecision::SelectBranch as i32,
                    selected_branch_index: accept_branch.branch_index,
                    ..Default::default()
                })),
            },
        );
        let private_looks = kaya_batch(&latest)
            .events
            .iter()
            .filter_map(|event| match event.ev.as_ref() {
                Some(Ev::Log(log)) if log.private_card_look.is_some() => Some(log),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(private_looks.len(), 1);
        assert_eq!(private_looks[0].visible_to_player_id, Some(29));
        assert_eq!(
            private_looks[0]
                .private_card_look
                .as_ref()
                .unwrap()
                .viewer_player_id,
            Some(29)
        );

        let placement = kaya_choice(&latest, ChoiceKind::LibraryTop)
            .expect("LibraryTop placement follows the opt-in and private look");
        assert_eq!(placement.deciding_player_id, 29);
        latest = kaya_player_command(
            &mut session,
            29,
            RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    resolution_prompt_id: placement.resolution_prompt_id,
                    ..Default::default()
                })),
            },
        );
        assert!(session
            .engine
            .as_ref()
            .unwrap()
            .state
            .pending_resolution
            .is_none());
        assert!(kaya_batch(&latest)
            .events
            .iter()
            .all(|event| !matches!(event.ev, Some(Ev::ResolutionChoiceRequired(_)))));
    }

    #[test]
    fn ripples_actual_card_casts_from_normal_offers_and_phases_out_chasm_skulker() {
        const CHASM_NAME: &str = "Chasm Skulker";
        const CHASM_CARD_ID: &str = "chasm_skulker";
        const RIPPLES_NAME: &str = "Ripples of Potential";
        const RIPPLES_CARD_ID: &str = "ripples_of_potential";

        let make_deck = |player_id: i32, pilot: bool| {
            let mut mainboard_card_name = if pilot {
                vec![CHASM_NAME.to_string(); 4]
            } else {
                Vec::new()
            };
            if pilot {
                mainboard_card_name.extend(vec![RIPPLES_NAME.to_string(); 4]);
            }
            mainboard_card_name.extend(vec![
                if pilot { "Island" } else { "Forest" }.to_string();
                60 - mainboard_card_name.len()
            ]);
            PlayerDeck {
                player_id,
                mainboard_card_name,
                commander_setup: Some(CommanderSetup {
                    declaration: Some(commander_setup::Declaration::NoCommander(true)),
                }),
            }
        };
        let player_decks = vec![make_deck(17, true), make_deck(29, false)];
        assert_eq!(player_decks[0].mainboard_card_name.len(), 60);
        assert_eq!(player_decks[1].mainboard_card_name.len(), 60);

        let mut session = EngineSession::new(false);
        let validation = session
            .process(&IpcEnvelope {
                msg: Some(Msg::ValidateDeck(ValidateDeck {
                    card_names: vec![
                        CHASM_NAME.into(),
                        RIPPLES_NAME.into(),
                        "Island".into(),
                        "Forest".into(),
                    ],
                })),
            })
            .expect("deck validation keeps the normal Ripples session open");
        assert!(
            validation.ok,
            "complete normal Ripples decks validate: {}",
            validation.error
        );
        assert!(validation.missing_card_names.is_empty());

        let mut latest = session
            .process(&IpcEnvelope {
                msg: Some(Msg::SessionStart(SessionStart {
                    game_id: 7_281_033,
                    seed: 1,
                    player_ids: vec![17, 29],
                    player_decks,
                    dev_commands_enabled: false,
                    diagnostic_capture_enabled: false,
                    ..Default::default()
                })),
            })
            .expect("SessionStart keeps the normal Ripples session open");
        assert!(latest.ok, "normal Ripples GameStart: {}", latest.error);
        assert!(!latest.effective_dev_commands_enabled);

        let starting = kaya_starting_player_offer(&latest)
            .expect("GameStart publishes the eligible starting-player offer");
        assert!(starting.eligible_starting_player_ids.contains(&17));
        latest = kaya_player_command(
            &mut session,
            starting.deciding_player_id,
            RuledCommand {
                cmd: Some(Cmd::ChooseStartingPlayer(ChooseStartingPlayer {
                    starting_player_id: 17,
                })),
            },
        );
        while let Some(opening) = kaya_mulligan_offer(&latest) {
            latest = kaya_player_command(
                &mut session,
                opening.deciding_player_id,
                RuledCommand {
                    cmd: Some(Cmd::Mulligan(MulliganDecision { keep: true })),
                },
            );
        }
        for _ in 0..4 {
            latest = ripples_pilot_pass_priority(&mut session, latest);
        }
        {
            let state = &session.engine.as_ref().unwrap().state;
            assert_eq!(state.turn_step, tricerules_core::TurnStep::Main1);
            assert_eq!(state.players[state.active_player_idx].id, 17);
        }

        let mut battlefield = Vec::new();
        kaya_capture_battlefield(&latest, 17, &mut battlefield);
        let chasm_from_view = |objects: &[BattlefieldObject]| {
            objects
                .iter()
                .find(|object| object.card_id == CHASM_CARD_ID)
                .cloned()
        };
        let mut chasm_cast = false;
        for _ in 0..8_192 {
            let (active_player_id, priority_player_id, main_phase) = {
                let state = &session.engine.as_ref().unwrap().state;
                (
                    state.players[state.active_player_idx].id,
                    state.priority_player_id(),
                    state.turn_step == tricerules_core::TurnStep::Main1,
                )
            };
            if active_player_id == 17 && priority_player_id == 17 && main_phase {
                let actions = kaya_batch(&latest)
                    .legal_by_player
                    .get(&17)
                    .expect("active pilot receives current legal actions")
                    .clone();
                if let Some(land) = actions
                    .hand_actions
                    .iter()
                    .find(|offer| offer.kind == HandActionKind::HandActionPlayLand as i32)
                    .cloned()
                {
                    latest = kaya_player_command(
                        &mut session,
                        17,
                        RuledCommand {
                            cmd: Some(Cmd::PlayLand(PlayLand {
                                source: Some(LandSource {
                                    location: Some(
                                        tricerules_proto::ruled::v1::land_source::Location::
                                            HandIndex(land.hand_index),
                                    ),
                                    ..Default::default()
                                }),
                                face_index: land.face_index,
                                ..Default::default()
                            })),
                        },
                    );
                    kaya_capture_battlefield(&latest, 17, &mut battlefield);
                    continue;
                }

                let has_chasm_offer = actions.hand_actions.iter().any(|offer| {
                    offer.kind == HandActionKind::HandActionCastSpell as i32
                        && offer.card_name == CHASM_NAME
                });
                let blue_sources = kaya_available_mana_abilities(&battlefield)
                    .into_iter()
                    .filter(|source| kaya_output_has_color(&source.3, 'U'))
                    .count();
                if has_chasm_offer && blue_sources >= 3 {
                    latest = ripples_pilot_cast_from_current_islands(
                        &mut session,
                        latest,
                        &mut battlefield,
                        17,
                        CHASM_NAME,
                        3,
                    );
                    for _ in 0..8 {
                        kaya_capture_battlefield(&latest, 17, &mut battlefield);
                        if chasm_from_view(&battlefield).is_some() {
                            chasm_cast = true;
                            break;
                        }
                        latest = ripples_pilot_pass_priority(&mut session, latest);
                    }
                    assert!(
                        chasm_cast,
                        "Chasm resolves through ordinary priority passes"
                    );
                    break;
                }
            }
            latest = ripples_pilot_pass_priority(&mut session, latest);
            kaya_capture_battlefield(&latest, 17, &mut battlefield);
        }
        assert!(chasm_cast, "ordinary current offers eventually cast Chasm");

        let chasm = chasm_from_view(&battlefield).expect("current view publishes Chasm");
        let chasm_object_id = chasm.object_id;
        let chasm_generation = chasm.zone_change_generation;
        let chasm_counter_total = |session: &EngineSession| {
            session
                .engine
                .as_ref()
                .expect("Ripples session has an engine")
                .state
                .objects
                .get(&chasm_object_id)
                .expect("Chasm keeps its engine object identity")
                .counter_count(tricerules_cards::primitives::CounterKind::PlusOnePlusOne)
        };
        {
            let state = &session.engine.as_ref().unwrap().state;
            assert_eq!(
                state
                    .zone_change_generation
                    .get(&chasm_object_id)
                    .copied()
                    .unwrap_or(0),
                chasm_generation,
                "omitted generations use the engine's zero default"
            );
            assert_eq!(chasm_counter_total(&session), 0);
        }
        let (mut previous_hand_size, mut previous_library_size) = {
            let state = &session.engine.as_ref().unwrap().state;
            let player = state.player_idx(17).expect("pilot is in the session");
            (
                state.players[player].hand.len(),
                state.players[player].library.len(),
            )
        };
        let mut natural_draw_observed = false;
        let mut draw_trigger_resolved = false;
        for _ in 0..8_192 {
            latest = ripples_pilot_pass_priority(&mut session, latest);
            kaya_capture_battlefield(&latest, 17, &mut battlefield);
            let state = &session.engine.as_ref().unwrap().state;
            let player = state.player_idx(17).expect("pilot remains in the session");
            let hand_size = state.players[player].hand.len();
            let library_size = state.players[player].library.len();
            if !natural_draw_observed
                && hand_size == previous_hand_size + 1
                && library_size.checked_add(1) == Some(previous_library_size)
            {
                natural_draw_observed = true;
            }
            previous_hand_size = hand_size;
            previous_library_size = library_size;
            let counter_total = chasm_counter_total(&session);
            if natural_draw_observed && counter_total == 1 {
                let at_pilot_main = state.players[state.active_player_idx].id == 17
                    && state.priority_player_id() == 17
                    && state.turn_step == tricerules_core::TurnStep::Main1;
                if at_pilot_main && state.stack.is_empty() && state.pending_resolution.is_none() {
                    draw_trigger_resolved = true;
                    break;
                }
            }
        }
        assert!(
            natural_draw_observed,
            "a later ordinary P17 draw moves one card from library to hand"
        );
        assert!(
            draw_trigger_resolved,
            "Chasm's real draw trigger resolves before the pilot Main1"
        );
        assert_eq!(chasm_counter_total(&session), 1);

        let mut ripples_cast = false;
        for _ in 0..8_192 {
            let (active_player_id, priority_player_id, main_phase) = {
                let state = &session.engine.as_ref().unwrap().state;
                (
                    state.players[state.active_player_idx].id,
                    state.priority_player_id(),
                    state.turn_step == tricerules_core::TurnStep::Main1,
                )
            };
            if active_player_id == 17 && priority_player_id == 17 && main_phase {
                let actions = kaya_batch(&latest)
                    .legal_by_player
                    .get(&17)
                    .expect("active pilot receives current legal actions")
                    .clone();
                if let Some(land) = actions
                    .hand_actions
                    .iter()
                    .find(|offer| offer.kind == HandActionKind::HandActionPlayLand as i32)
                    .cloned()
                {
                    latest = kaya_player_command(
                        &mut session,
                        17,
                        RuledCommand {
                            cmd: Some(Cmd::PlayLand(PlayLand {
                                source: Some(LandSource {
                                    location: Some(
                                        tricerules_proto::ruled::v1::land_source::Location::
                                            HandIndex(land.hand_index),
                                    ),
                                    ..Default::default()
                                }),
                                face_index: land.face_index,
                                ..Default::default()
                            })),
                        },
                    );
                    kaya_capture_battlefield(&latest, 17, &mut battlefield);
                    continue;
                }
                let has_ripples_offer = actions.hand_actions.iter().any(|offer| {
                    offer.kind == HandActionKind::HandActionCastSpell as i32
                        && offer.card_name == RIPPLES_NAME
                });
                let blue_sources = kaya_available_mana_abilities(&battlefield)
                    .into_iter()
                    .filter(|source| kaya_output_has_color(&source.3, 'U'))
                    .count();
                if has_ripples_offer && blue_sources >= 2 {
                    latest = ripples_pilot_cast_from_current_islands(
                        &mut session,
                        latest,
                        &mut battlefield,
                        17,
                        RIPPLES_NAME,
                        2,
                    );
                    ripples_cast = true;
                    break;
                }
            }
            latest = ripples_pilot_pass_priority(&mut session, latest);
            kaya_capture_battlefield(&latest, 17, &mut battlefield);
        }
        assert!(
            ripples_cast,
            "ordinary current offers eventually cast Ripples"
        );
        assert_eq!(chasm_counter_total(&session), 1);
        let ripples_stack_id = session
            .engine
            .as_ref()
            .unwrap()
            .state
            .stack
            .last()
            .expect("actual Ripples is awaiting normal resolution")
            .id;
        assert_eq!(
            session.engine.as_ref().unwrap().state.objects[&ripples_stack_id].card_id,
            RIPPLES_CARD_ID
        );

        let mut resolution_batches = Vec::new();
        let mut proliferate_offer = None;
        for _ in 0..8 {
            latest = ripples_pilot_pass_priority(&mut session, latest);
            let batch = kaya_batch(&latest).clone();
            proliferate_offer = kaya_choice(&latest, ChoiceKind::Proliferate);
            resolution_batches.push(batch);
            if proliferate_offer.is_some() {
                break;
            }
        }
        let proliferate_offer = proliferate_offer
            .expect("a returned post-command batch publishes the Proliferate choice");
        assert!(!resolution_batches.is_empty());
        assert_ne!(proliferate_offer.resolution_prompt_id, 0);
        assert_eq!(proliferate_offer.deciding_player_id, 17);
        assert_eq!((proliferate_offer.min, proliferate_offer.max), (0, 1));
        assert_eq!(
            proliferate_offer.candidate_object_ids,
            vec![chasm_object_id],
            "the real counter-bearing Chasm is the current Proliferate recipient"
        );
        assert_eq!(chasm_counter_total(&session), 1);

        latest = kaya_player_command(
            &mut session,
            17,
            RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    resolution_prompt_id: proliferate_offer.resolution_prompt_id,
                    chosen_object_ids: vec![chasm_object_id],
                    decision: ResolutionChoiceDecision::Unspecified as i32,
                    ..Default::default()
                })),
            },
        );
        resolution_batches.push(kaya_batch(&latest).clone());
        let permanent_offer = kaya_choice(&latest, ChoiceKind::PermanentObjects)
            .expect("the response to Proliferate immediately publishes the phase-out subset");
        assert_ne!(permanent_offer.resolution_prompt_id, 0);
        assert_ne!(
            permanent_offer.resolution_prompt_id, proliferate_offer.resolution_prompt_id,
            "the second choice has a fresh prompt ID"
        );
        assert_eq!(permanent_offer.deciding_player_id, 17);
        assert_eq!((permanent_offer.min, permanent_offer.max), (0, 1));
        assert_eq!(
            permanent_offer.candidate_object_ids,
            vec![chasm_object_id],
            "the phase-out subset refers to the same receipted Chasm"
        );
        assert_eq!(
            chasm_counter_total(&session),
            2,
            "Proliferate increments the counter before the second choice"
        );
        {
            let state = &session.engine.as_ref().unwrap().state;
            let parked_stack_id = state
                .pending_resolution
                .as_ref()
                .and_then(|pending| pending.continuation.stack())
                .map(|stack| stack.item.id);
            assert_eq!(parked_stack_id, Some(ripples_stack_id));
        }

        latest = kaya_player_command(
            &mut session,
            17,
            RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    resolution_prompt_id: permanent_offer.resolution_prompt_id,
                    chosen_object_ids: vec![chasm_object_id],
                    decision: ResolutionChoiceDecision::Unspecified as i32,
                    ..Default::default()
                })),
            },
        );
        let final_batch = kaya_batch(&latest);
        assert!(final_batch.events.iter().any(|event| {
            matches!(
                event.ev.as_ref(),
                Some(Ev::StackResolved(resolved)) if resolved.object_id == ripples_stack_id
            )
        }));
        assert!(final_batch
            .events
            .iter()
            .all(|event| !matches!(event.ev, Some(Ev::ResolutionChoiceRequired(_)))));
        let state = &session.engine.as_ref().unwrap().state;
        assert!(
            state.stack.is_empty(),
            "Ripples completed its stack resolution"
        );
        assert!(
            state.pending_resolution.is_none(),
            "Ripples continuation completed"
        );
        let chasm_object = state
            .objects
            .get(&chasm_object_id)
            .expect("phasing preserves the Chasm object identity");
        assert!(
            chasm_object.phase_status.is_some(),
            "the selected Chasm is actually phased out"
        );
        assert_eq!(
            state
                .zone_change_generation
                .get(&chasm_object_id)
                .copied()
                .unwrap_or(0),
            chasm_generation,
            "phasing preserves Chasm's zone-change generation"
        );
        assert_eq!(
            chasm_object.counter_count(tricerules_cards::primitives::CounterKind::PlusOnePlusOne),
            2,
            "phasing preserves both +1/+1 counters"
        );
    }
    fn narset_normal_fixed_decks() -> Vec<PlayerDeck> {
        let mut pilot = Vec::new();
        pilot.extend(vec!["Narset Transcendent".to_string(); 4]);
        pilot.extend(vec!["Divination".to_string(); 4]);
        pilot.extend(vec!["Island".to_string(); 26]);
        pilot.extend(vec!["Plains".to_string(); 26]);

        let mut opponent = Vec::new();
        opponent.extend(vec!["Divination".to_string(); 4]);
        opponent.extend(vec!["Ornithopter".to_string(); 4]);
        opponent.extend(vec!["Island".to_string(); 52]);

        vec![
            PlayerDeck {
                player_id: TEKUTHAL_NORMAL_PILOT_ID,
                mainboard_card_name: pilot,
                commander_setup: Some(CommanderSetup {
                    declaration: Some(commander_setup::Declaration::NoCommander(true)),
                }),
            },
            PlayerDeck {
                player_id: TEKUTHAL_NORMAL_OPPONENT_ID,
                mainboard_card_name: opponent,
                commander_setup: Some(CommanderSetup {
                    declaration: Some(commander_setup::Declaration::NoCommander(true)),
                }),
            },
        ]
    }

    fn narset_normal_assert_decks(player_decks: &[PlayerDeck]) {
        let registry = tricerules_cards::registry::global();
        assert_eq!(player_decks.len(), 2);
        for (deck, expected_player_id) in player_decks
            .iter()
            .zip([TEKUTHAL_NORMAL_PILOT_ID, TEKUTHAL_NORMAL_OPPONENT_ID])
        {
            assert_eq!(deck.player_id, expected_player_id);
            assert_eq!(deck.mainboard_card_name.len(), 60);
            assert!(matches!(
                deck.commander_setup
                    .as_ref()
                    .and_then(|setup| setup.declaration.as_ref()),
                Some(commander_setup::Declaration::NoCommander(true))
            ));
            for card_name in deck
                .mainboard_card_name
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
            {
                assert!(
                    registry.id_for_name(card_name).is_some(),
                    "Narset normal deck card name is absent from the registry: {card_name}"
                );
                let copies = deck
                    .mainboard_card_name
                    .iter()
                    .filter(|candidate| *candidate == card_name)
                    .count();
                if matches!(card_name.as_str(), "Island" | "Plains") {
                    assert!(copies > 4, "basics are not subject to the nonbasic cap");
                } else {
                    assert!(copies <= 4, "too many copies of nonbasic {card_name}");
                }
            }
        }
    }

    fn narset_normal_player_command(
        session: &mut EngineSession,
        directory: &Path,
        rows: &mut Vec<String>,
        accepted_commands: &mut usize,
        player_id: i32,
        command: RuledCommand,
        battlefield: &mut Vec<BattlefieldObject>,
    ) -> IpcResponse {
        assert!(
            *accepted_commands < 8_192,
            "Narset normal pilot stays inside the accepted-command bound"
        );
        tekuthal_normal_player_command(
            session,
            directory,
            rows,
            accepted_commands,
            player_id,
            command,
            battlefield,
        )
    }

    fn narset_normal_advance_to_main(
        session: &mut EngineSession,
        directory: &Path,
        rows: &mut Vec<String>,
        accepted_commands: &mut usize,
        latest: IpcResponse,
        battlefield: &mut Vec<BattlefieldObject>,
        target_player_id: i32,
    ) -> IpcResponse {
        narset_normal_advance_to_step(
            session,
            directory,
            rows,
            accepted_commands,
            latest,
            battlefield,
            target_player_id,
            tricerules_core::TurnStep::Main1,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn narset_normal_advance_to_step(
        session: &mut EngineSession,
        directory: &Path,
        rows: &mut Vec<String>,
        accepted_commands: &mut usize,
        mut latest: IpcResponse,
        battlefield: &mut Vec<BattlefieldObject>,
        target_player_id: i32,
        target_step: tricerules_core::TurnStep,
    ) -> IpcResponse {
        for _ in 0..8_192 {
            let state = &session
                .engine
                .as_ref()
                .expect("Narset normal session has an engine")
                .state;
            if state.pending_resolution.is_some() {
                return latest;
            }
            if state.turn_step == target_step
                && state.players[state.active_player_idx].id == target_player_id
                && state.priority_player_id() == target_player_id
                && state.stack.is_empty()
            {
                kaya_capture_battlefield(&latest, target_player_id, battlefield);
                return latest;
            }

            let mut priority_player = state.priority_player_id();
            let discard_count = if state.turn_step == tricerules_core::TurnStep::Cleanup
                && !state.cleanup_priority_active
                && state.cleanup_discard_player == Some(priority_player)
            {
                state
                    .players
                    .iter()
                    .find(|player| player.id == priority_player)
                    .expect("cleanup actor is in the game")
                    .hand
                    .len()
                    .saturating_sub(7)
            } else {
                0
            };
            if discard_count > 0 {
                let legal_actions = kaya_batch(&latest)
                    .legal_by_player
                    .get(&priority_player)
                    .expect("current cleanup actor receives legal hand actions");
                let mut cleanup_offers = legal_actions
                    .hand_actions
                    .iter()
                    .filter(|offer| {
                        let protected = (priority_player == TEKUTHAL_NORMAL_PILOT_ID
                            && matches!(
                                offer.card_name.as_str(),
                                "Narset Transcendent" | "Divination"
                            ))
                            || (priority_player == TEKUTHAL_NORMAL_OPPONENT_ID
                                && matches!(
                                    offer.card_name.as_str(),
                                    "Divination" | "Ornithopter"
                                ));
                        offer.kind == HandActionKind::HandActionCleanupDiscard as i32 && !protected
                    })
                    .map(|offer| offer.hand_index)
                    .collect::<Vec<_>>();
                cleanup_offers.sort_unstable();
                cleanup_offers.truncate(discard_count);
                assert_eq!(
                    cleanup_offers.len(),
                    discard_count,
                    "current cleanup offers preserve Narset, Divination, and Ornithopter"
                );
                latest = narset_normal_player_command(
                    session,
                    directory,
                    rows,
                    accepted_commands,
                    priority_player,
                    RuledCommand {
                        cmd: Some(Cmd::DiscardToHandSize(DiscardToHandSize {
                            hand_card_indices: cleanup_offers,
                        })),
                    },
                    battlefield,
                );
                priority_player = session
                    .engine
                    .as_ref()
                    .expect("Narset session has an engine after cleanup discard")
                    .state
                    .priority_player_id();
                assert!(kaya_batch(&latest)
                    .legal_by_player
                    .contains_key(&priority_player));
            }

            latest = narset_normal_player_command(
                session,
                directory,
                rows,
                accepted_commands,
                priority_player,
                RuledCommand {
                    cmd: Some(Cmd::PassPriority(PassPriority {})),
                },
                battlefield,
            );
        }
        panic!("Narset normal priority passes did not reach player {target_player_id}'s {target_step:?}");
    }

    #[allow(clippy::too_many_arguments)]
    fn narset_normal_play_land(
        session: &mut EngineSession,
        directory: &Path,
        rows: &mut Vec<String>,
        accepted_commands: &mut usize,
        latest: &IpcResponse,
        battlefield: &mut Vec<BattlefieldObject>,
        player_id: i32,
        p17_islands: &mut usize,
        p17_plains: &mut usize,
    ) -> IpcResponse {
        let offers = kaya_batch(latest)
            .legal_by_player
            .get(&player_id)
            .expect("active player receives current land offers")
            .hand_actions
            .iter()
            .filter(|offer| offer.kind == HandActionKind::HandActionPlayLand as i32)
            .collect::<Vec<_>>();
        let selected = if player_id == TEKUTHAL_NORMAL_PILOT_ID && *p17_islands == 0 {
            offers
                .iter()
                .filter(|offer| offer.card_name == "Island")
                .min_by_key(|offer| offer.hand_index)
                .expect("P17's current hand offers the missing first Island")
        } else if player_id == TEKUTHAL_NORMAL_PILOT_ID && *p17_plains == 0 {
            offers
                .iter()
                .filter(|offer| offer.card_name == "Plains")
                .min_by_key(|offer| offer.hand_index)
                .expect("P17's current hand offers the missing Plains")
        } else if player_id == TEKUTHAL_NORMAL_OPPONENT_ID {
            offers
                .iter()
                .filter(|offer| offer.card_name == "Island")
                .min_by_key(|offer| offer.hand_index)
                .expect("P29's current hand offers an ordinary Island")
        } else {
            offers
                .iter()
                .min_by_key(|offer| offer.hand_index)
                .expect("P17's current hand offers an ordinary basic land")
        };
        let card_name = selected.card_name.to_string();
        let hand_index = selected.hand_index;
        let response = narset_normal_player_command(
            session,
            directory,
            rows,
            accepted_commands,
            player_id,
            RuledCommand {
                cmd: Some(Cmd::PlayLand(PlayLand {
                    source: Some(LandSource {
                        location: Some(
                            tricerules_proto::ruled::v1::land_source::Location::HandIndex(
                                hand_index,
                            ),
                        ),
                        ..Default::default()
                    }),
                    ..Default::default()
                })),
            },
            battlefield,
        );
        kaya_capture_battlefield(&response, player_id, battlefield);
        assert!(response.ok, "current {card_name} land offer is accepted");
        if player_id == TEKUTHAL_NORMAL_PILOT_ID {
            match card_name.as_str() {
                "Island" => *p17_islands += 1,
                "Plains" => *p17_plains += 1,
                _ => panic!("P17's frozen land plan only plays Island or Plains"),
            }
        }
        response
    }

    #[allow(clippy::too_many_arguments)]
    fn narset_normal_cast_from_current_offer(
        session: &mut EngineSession,
        directory: &Path,
        rows: &mut Vec<String>,
        accepted_commands: &mut usize,
        mut latest: IpcResponse,
        battlefield: &mut Vec<BattlefieldObject>,
        card_name: &str,
        expected_total_cost: &str,
        blue_mana: u32,
        white_mana: u32,
        expected_source_object_id: Option<u32>,
        resolve_after_commit: bool,
    ) -> IpcResponse {
        let p17_hand = &session
            .engine
            .as_ref()
            .expect("Narset session has an engine before the offered cast")
            .state
            .players
            .iter()
            .find(|player| player.id == TEKUTHAL_NORMAL_PILOT_ID)
            .expect("P17 owns the current hand offer")
            .hand;
        let spell = kaya_batch(&latest)
            .legal_by_player
            .get(&TEKUTHAL_NORMAL_PILOT_ID)
            .expect("P17 receives the current legal hand actions")
            .hand_actions
            .iter()
            .find(|offer| {
                offer.kind == HandActionKind::HandActionCastSpell as i32
                    && offer.card_name == card_name
                    && expected_source_object_id.is_none_or(|expected| {
                        usize::try_from(offer.hand_index)
                            .ok()
                            .and_then(|index| p17_hand.get(index))
                            == Some(&expected)
                    })
            })
            .cloned()
            .unwrap_or_else(|| panic!("current P17 hand offer includes castable {card_name}"));
        let (source_object_id, source_generation) = {
            let state = &session
                .engine
                .as_ref()
                .expect("Narset session has an engine before the offered cast")
                .state;
            let player = state
                .players
                .iter()
                .find(|player| player.id == TEKUTHAL_NORMAL_PILOT_ID)
                .expect("P17 owns the current hand offer");
            let object_id = *player
                .hand
                .get(usize::try_from(spell.hand_index).expect("hand index fits usize"))
                .expect("current hand offer resolves to a physical card");
            let object = state
                .objects
                .get(&object_id)
                .expect("current hand offer resolves to a game object");
            assert_eq!(object.zone, tricerules_core::Zone::Hand);
            assert_eq!(
                object.card_id,
                tricerules_cards::registry::global()
                    .id_for_name(card_name)
                    .expect("the offered card name is in the global registry")
            );
            (
                object_id,
                state
                    .zone_change_generation
                    .get(&object_id)
                    .copied()
                    .unwrap_or(0),
            )
        };
        if card_name == "Narset Transcendent" {
            assert_eq!(
                (source_object_id, source_generation, spell.face_index),
                (30, 1, 0)
            );
        }
        if let Some(expected) = expected_source_object_id {
            assert_eq!(source_object_id, expected);
        }
        println!(
            "{card_name} current hand offer source ObjectId={source_object_id} generation={source_generation} face_index={} cast_method=Normal",
            spell.face_index
        );
        latest = narset_normal_player_command(
            session,
            directory,
            rows,
            accepted_commands,
            TEKUTHAL_NORMAL_PILOT_ID,
            RuledCommand {
                cmd: Some(Cmd::BeginSpellCast(BeginSpellCast {
                    announcement: Some(tricerules_proto::ruled::v1::SpellCastAnnouncement {
                        source: Some(CastSource {
                            location: Some(
                                tricerules_proto::ruled::v1::cast_source::Location::HandIndex(
                                    spell.hand_index,
                                ),
                            ),
                            expected_zone_change_generation: None,
                        }),
                        face_index: spell.face_index,
                        cast_cost_group_selections: Vec::new(),
                        cast_method: CastMethod::Normal as i32,
                        ..Default::default()
                    }),
                })),
            },
            battlefield,
        );
        let pending = kaya_batch(&latest)
            .legal_by_player
            .get(&TEKUTHAL_NORMAL_PILOT_ID)
            .and_then(|actions| actions.pending_spell_cast.as_ref())
            .expect("current normal cast publishes its pending payment");
        let payment_preview = pending
            .payment_preview
            .as_ref()
            .expect("pending normal cast publishes its payment preview");
        assert_eq!(payment_preview.total_cost, expected_total_cost);
        let transaction_id = pending.transaction_id;

        let mut produced = PaymentMana::default();
        for (color, count) in [('W', white_mana), ('U', blue_mana)] {
            for _ in 0..count {
                let (object_id, generation, ability_index, output) = battlefield
                    .iter()
                    .filter(|permanent| {
                        !permanent.tapped
                            && matches!(permanent.card_id.as_str(), "island" | "plains")
                    })
                    .flat_map(|permanent| {
                        permanent
                            .activated_abilities
                            .iter()
                            .filter(|ability| ability.is_mana_ability && ability.activatable)
                            .map(|ability| {
                                (
                                    permanent.object_id,
                                    permanent.zone_change_generation,
                                    ability.ability_index,
                                    ability.mana_produced.clone(),
                                )
                            })
                    })
                    .find(|source| kaya_output_has_color(&source.3, color))
                    .unwrap_or_else(|| {
                        panic!("current untapped Island/Plains offers produce {color}")
                    });
                narset_normal_player_command(
                    session,
                    directory,
                    rows,
                    accepted_commands,
                    TEKUTHAL_NORMAL_PILOT_ID,
                    RuledCommand {
                        cmd: Some(Cmd::ActivateAbility(ActivateAbility {
                            source_object_id: object_id,
                            ability_index,
                            source_zone: AbilitySourceZone::Battlefield as i32,
                            expected_zone_change_generation: generation,
                            ..Default::default()
                        })),
                    },
                    battlefield,
                );
                if color == 'U' {
                    produced.u += 1;
                } else {
                    produced.w += 1;
                }
                let state = &session
                    .engine
                    .as_ref()
                    .expect("Narset session remains open after current mana ability")
                    .state;
                let player = state
                    .players
                    .iter()
                    .find(|player| player.id == TEKUTHAL_NORMAL_PILOT_ID)
                    .expect("P17 remains in the current normal session");
                assert_eq!(player.mana_pool.blue, produced.u);
                assert_eq!(player.mana_pool.white, produced.w);
                assert_eq!(player.mana_pool.colorless, 0);
                println!(
                    "{card_name} current {color} source ObjectId={object_id} generation={generation} ability={ability_index} output={output}; pool={:?}",
                    player.mana_pool
                );
            }
        }

        let current_revision = {
            let state = &session
                .engine
                .as_ref()
                .expect("Narset session has an engine after current mana abilities")
                .state;
            let pending = state
                .pending_spell_cast
                .as_ref()
                .expect("current mana abilities preserve the pending spell cast");
            assert_eq!(pending.transaction_id, transaction_id);
            assert_eq!(pending.caster, TEKUTHAL_NORMAL_PILOT_ID);
            assert_eq!(pending.reserved_object_id, source_object_id);
            state.command_index
        };
        let payment_request = IpcEnvelope {
            msg: Some(Msg::PaymentQuery(PaymentQuery {
                player_id: TEKUTHAL_NORMAL_PILOT_ID,
                preview: Some(PreviewPayment {
                    transaction_id,
                    revision: current_revision,
                    commit_spell_cast: Some(CommitSpellCast {
                        transaction_id,
                        payment: Some(PaymentSelection {
                            expected_state_revision: current_revision,
                            mana: Some(produced),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
            })),
        };
        let payment_response = session
            .process(&payment_request)
            .expect("PaymentQuery keeps the Narset normal session open");
        pair(directory, rows, &payment_request, &payment_response);
        assert!(
            payment_response.ok,
            "{card_name} PaymentQuery: {}",
            payment_response.error
        );
        let preview = payment_response
            .batch
            .as_ref()
            .and_then(|batch| batch.payment_preview.as_ref())
            .expect("PaymentQuery returns the current payment preview");
        assert!(
            preview.valid && preview.complete,
            "{card_name}: {preview:?}"
        );
        assert_eq!(preview.transaction_id, transaction_id);
        assert_eq!(preview.total_cost, expected_total_cost);
        let selection = preview
            .selection
            .clone()
            .expect("complete PaymentQuery returns its exact current selection");
        assert_eq!(selection.expected_state_revision, current_revision);
        assert_eq!(selection.mana.as_ref(), Some(&produced));
        let restricted_mana = preview.restricted_mana.clone();
        println!(
            "{card_name} PaymentQuery transaction={transaction_id} revision={current_revision} returned_selection={selection:?} restricted_mana={restricted_mana:?}"
        );
        latest = narset_normal_player_command(
            session,
            directory,
            rows,
            accepted_commands,
            TEKUTHAL_NORMAL_PILOT_ID,
            RuledCommand {
                cmd: Some(Cmd::CommitSpellCast(CommitSpellCast {
                    transaction_id,
                    payment: Some(selection),
                    restricted_mana,
                })),
            },
            battlefield,
        );
        assert!(session
            .engine
            .as_ref()
            .expect("Narset session has an engine after payment")
            .state
            .pending_spell_cast
            .is_none());
        if resolve_after_commit {
            narset_normal_resolve_stack(
                session,
                directory,
                rows,
                accepted_commands,
                latest,
                battlefield,
            )
        } else {
            latest
        }
    }

    fn narset_normal_resolve_stack(
        session: &mut EngineSession,
        directory: &Path,
        rows: &mut Vec<String>,
        accepted_commands: &mut usize,
        mut latest: IpcResponse,
        battlefield: &mut Vec<BattlefieldObject>,
    ) -> IpcResponse {
        for _ in 0..16 {
            let state = &session
                .engine
                .as_ref()
                .expect("Narset normal session has an engine")
                .state;
            if state.stack.is_empty() {
                return latest;
            }
            let priority_player = state.priority_player_id();
            latest = narset_normal_player_command(
                session,
                directory,
                rows,
                accepted_commands,
                priority_player,
                RuledCommand {
                    cmd: Some(Cmd::PassPriority(PassPriority {})),
                },
                battlefield,
            );
        }
        assert!(session
            .engine
            .as_ref()
            .expect("Narset normal session has an engine")
            .state
            .stack
            .is_empty());
        latest
    }

    fn narset_normal_resolve_top_stack(
        session: &mut EngineSession,
        directory: &Path,
        rows: &mut Vec<String>,
        accepted_commands: &mut usize,
        mut latest: IpcResponse,
        battlefield: &mut Vec<BattlefieldObject>,
    ) -> IpcResponse {
        let top_id = session
            .engine
            .as_ref()
            .expect("Narset normal session has an engine before top resolution")
            .state
            .stack
            .last()
            .expect("Narset has a current top stack item to resolve")
            .id;
        for _ in 0..16 {
            let state = &session
                .engine
                .as_ref()
                .expect("Narset normal session has an engine during top resolution")
                .state;
            if !state.stack.iter().any(|item| item.id == top_id) {
                return latest;
            }
            let priority_player = state.priority_player_id();
            latest = narset_normal_player_command(
                session,
                directory,
                rows,
                accepted_commands,
                priority_player,
                RuledCommand {
                    cmd: Some(Cmd::PassPriority(PassPriority {})),
                },
                battlefield,
            );
        }
        panic!("Narset top stack item {top_id} did not resolve within the priority bound");
    }

    fn narset_normal_activate_loyalty(
        session: &mut EngineSession,
        directory: &Path,
        rows: &mut Vec<String>,
        accepted_commands: &mut usize,
        battlefield: &mut Vec<BattlefieldObject>,
        ability_id: &str,
    ) -> IpcResponse {
        let (source_object_id, source_generation, ability_index, activatable) = {
            let narset = battlefield
                .iter()
                .find(|permanent| permanent.card_id == "narset_transcendent")
                .expect("the current Narset battlefield view is present before activation");
            let ability = narset
                .activated_abilities
                .iter()
                .find(|ability| {
                    ability.presentation.as_ref().is_some_and(|presentation| {
                        presentation.path.iter().any(|part| {
                            part.kind == PresentationPathKind::Ability as i32
                                && part.id == ability_id
                        })
                    })
                })
                .unwrap_or_else(|| panic!("current Narset view contains {ability_id}"));
            (
                narset.object_id,
                narset.zone_change_generation,
                ability.ability_index,
                ability.activatable,
            )
        };
        assert!(activatable, "current Narset view offers {ability_id}");
        narset_normal_player_command(
            session,
            directory,
            rows,
            accepted_commands,
            TEKUTHAL_NORMAL_PILOT_ID,
            RuledCommand {
                cmd: Some(Cmd::ActivateAbility(ActivateAbility {
                    source_object_id,
                    ability_index,
                    source_zone: AbilitySourceZone::Battlefield as i32,
                    expected_zone_change_generation: source_generation,
                    ..Default::default()
                })),
            },
            battlefield,
        )
    }

    fn narset_normal_state_field(session: &EngineSession, field: &str) -> Value {
        session
            .engine
            .as_ref()
            .expect("Narset normal session has an engine for state diagnostics")
            .diagnostic_snapshot()
            .expect("serialize Narset normal state diagnostics")["state"][field]
            .clone()
    }

    fn narset_normal_decline_current_look(
        session: &mut EngineSession,
        directory: &Path,
        rows: &mut Vec<String>,
        accepted_commands: &mut usize,
        latest: IpcResponse,
        battlefield: &mut Vec<BattlefieldObject>,
    ) -> IpcResponse {
        let choice = kaya_choice(&latest, ChoiceKind::LibraryLook)
            .expect("resolved current Narset +1 publishes its LibraryLook choice");
        assert_eq!(choice.deciding_player_id, TEKUTHAL_NORMAL_PILOT_ID);
        assert_ne!(choice.resolution_prompt_id, 0);
        assert_eq!(choice.min, 0);
        assert!(choice.max <= 1);
        assert!(kaya_batch(&latest)
            .events
            .iter()
            .all(|event| !matches!(event.ev, Some(Ev::CardsRevealed(_)))));
        narset_normal_player_command(
            session,
            directory,
            rows,
            accepted_commands,
            TEKUTHAL_NORMAL_PILOT_ID,
            RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    resolution_prompt_id: choice.resolution_prompt_id,
                    chosen_object_ids: Vec::new(),
                    decision: ResolutionChoiceDecision::Unspecified as i32,
                    ..Default::default()
                })),
            },
            battlefield,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn narset_normal_accept_rebound_offer(
        session: &mut EngineSession,
        directory: &Path,
        rows: &mut Vec<String>,
        accepted_commands: &mut usize,
        mut latest: IpcResponse,
        battlefield: &mut Vec<BattlefieldObject>,
        object_id: u32,
        exile_generation: u64,
    ) -> IpcResponse {
        let choice = kaya_choice(&latest, ChoiceKind::SpecialCast)
            .expect("the current Rebound trigger offers its exact SpecialCast choice");
        assert_eq!(choice.deciding_player_id, TEKUTHAL_NORMAL_PILOT_ID);
        assert_ne!(choice.resolution_prompt_id, 0);
        assert_eq!(choice.candidate_object_ids, vec![object_id]);
        assert_eq!(choice.candidate_selectable, [true]);
        let trigger_id = choice.source_object_id;
        let (permission_id, face_index) = {
            let engine = session
                .engine
                .as_ref()
                .expect("Narset normal session has an engine at the Rebound choice");
            let pending = engine
                .state
                .pending_resolution
                .as_ref()
                .expect("the current SpecialCast choice remains pending");
            assert_eq!(pending.deciding_player, TEKUTHAL_NORMAL_PILOT_ID);
            assert_eq!(pending.presentation.choice_kind, ChoiceKind::SpecialCast);
            assert_eq!(pending.presentation.candidates, vec![object_id]);
            assert_eq!(
                pending.resolution_prompt_id.get(),
                choice.resolution_prompt_id
            );
            let tricerules_core::state::ResolutionContinuation::SpecialCast {
                stack,
                exiled,
                face_index,
                method,
                cost,
                permission_scope,
                completion,
                ..
            } = &pending.continuation
            else {
                panic!("the current Rebound trigger owns the SpecialCast continuation");
            };
            assert_eq!(stack.item.id, trigger_id);
            assert!(stack.item.is_triggered);
            assert_eq!(stack.item.controller, TEKUTHAL_NORMAL_PILOT_ID);
            assert_eq!(exiled.object_id, object_id);
            assert_eq!(exiled.zone_change_generation, exile_generation);
            assert_eq!(*face_index, 0);
            assert_eq!(*method, tricerules_core::state::SpellCastMethod::Permission);
            assert_eq!(*cost, tricerules_cards::mana::ManaCost::default());
            assert_eq!(
                *permission_scope,
                Some(tricerules_core::state::ExilePlayPermissionScope::CastCard)
            );
            assert!(matches!(
                completion,
                tricerules_core::state::SpecialCastCompletion::ResumeStack
            ));
            assert!(engine
                .state
                .active_exile_play_permissions
                .iter()
                .all(|permission| permission.object_id != object_id));
            let action = kaya_batch(&latest)
                .legal_by_player
                .get(&TEKUTHAL_NORMAL_PILOT_ID)
                .expect("P17 receives the current Rebound offer")
                .zone_cast_actions
                .iter()
                .find(|action| action.object_id == object_id)
                .expect("the exact exiled Divination has a current cast offer");
            assert_eq!(action.card_name, "Divination");
            assert_eq!(
                action.source_zone,
                tricerules_proto::ruled::v1::CastSourceZone::Exile as i32
            );
            assert_eq!(action.zone_change_generation, exile_generation);
            assert_eq!(action.face_index, 0);
            assert_eq!(
                action.cast_method,
                tricerules_proto::ruled::v1::CastMethod::Permission as i32
            );
            assert_eq!(action.cost, "");
            let permission_id = action
                .casting_permission_id
                .expect("the current Rebound offer includes its exact permission ID");
            assert_ne!(permission_id, 0);
            (
                permission_id,
                u32::try_from(*face_index).expect("face index fits the wire"),
            )
        };

        latest = narset_normal_player_command(
            session,
            directory,
            rows,
            accepted_commands,
            TEKUTHAL_NORMAL_PILOT_ID,
            RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    resolution_prompt_id: choice.resolution_prompt_id,
                    decision: ResolutionChoiceDecision::CastSpell as i32,
                    spell_cast_announcement: Some(
                        tricerules_proto::ruled::v1::SpellCastAnnouncement {
                            source: Some(CastSource {
                                location: Some(
                                    tricerules_proto::ruled::v1::cast_source::Location::ExileObjectId(
                                        object_id,
                                    ),
                                ),
                                expected_zone_change_generation: Some(exile_generation),
                            }),
                            face_index,
                            cast_method: CastMethod::Permission as i32,
                            casting_permission_id: Some(permission_id),
                            ..Default::default()
                        },
                    ),
                    ..Default::default()
                })),
            },
            battlefield,
        );
        assert!(kaya_batch(&latest)
            .events
            .iter()
            .all(|event| !matches!(event.ev, Some(Ev::StackPushed(_)))));
        let (transaction_id, current_revision, mana_before) = {
            let engine = session
                .engine
                .as_ref()
                .expect("Narset normal session has an engine after accepting Rebound");
            let pending = engine
                .state
                .pending_spell_cast
                .as_ref()
                .expect("the accepted Rebound offer opens its staged payment");
            assert!(pending.resolution_time_offer);
            assert_eq!(pending.caster, TEKUTHAL_NORMAL_PILOT_ID);
            assert_eq!(pending.reserved_object_id, object_id);
            assert_eq!(pending.original_zone, tricerules_core::Zone::Exile);
            assert_eq!(pending.original_zone_change_generation, exile_generation);
            assert_eq!(
                pending.announcement.casting_permission_id,
                Some(permission_id)
            );
            let source = pending
                .announcement
                .source
                .as_ref()
                .expect("the staged Rebound cast keeps its exact Exile source");
            assert_eq!(
                source.location,
                Some(tricerules_proto::ruled::v1::cast_source::Location::ExileObjectId(object_id))
            );
            assert_eq!(
                source.expected_zone_change_generation,
                Some(exile_generation)
            );
            assert_eq!(pending.announcement.face_index, face_index);
            assert_eq!(
                pending.announcement.cast_method,
                CastMethod::Permission as i32
            );
            assert_eq!(
                engine.state.objects[&object_id].zone,
                tricerules_core::Zone::Stack
            );
            assert_eq!(
                engine.state.zone_change_generation.get(&object_id),
                Some(&(exile_generation + 1))
            );
            let p17 = engine
                .state
                .players
                .iter()
                .find(|player| player.id == TEKUTHAL_NORMAL_PILOT_ID)
                .expect("P17 remains in the game");
            (
                pending.transaction_id,
                engine.state.command_index,
                p17.mana_pool,
            )
        };
        let payment_request = IpcEnvelope {
            msg: Some(Msg::PaymentQuery(PaymentQuery {
                player_id: TEKUTHAL_NORMAL_PILOT_ID,
                preview: Some(PreviewPayment {
                    transaction_id,
                    revision: current_revision,
                    commit_spell_cast: Some(CommitSpellCast {
                        transaction_id,
                        payment: Some(PaymentSelection {
                            expected_state_revision: current_revision,
                            mana: Some(PaymentMana::default()),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
            })),
        };
        let payment_response = session
            .process(&payment_request)
            .expect("PaymentQuery keeps the Rebound normal session open");
        pair(directory, rows, &payment_request, &payment_response);
        assert!(
            payment_response.ok,
            "Rebound PaymentQuery: {}",
            payment_response.error
        );
        let preview = payment_response
            .batch
            .as_ref()
            .and_then(|batch| batch.payment_preview.as_ref())
            .expect("Rebound PaymentQuery returns the current payment preview");
        assert!(
            preview.valid && preview.complete,
            "Rebound zero-cost preview: {preview:?}"
        );
        assert_eq!(preview.transaction_id, transaction_id);
        assert_eq!(preview.revision, current_revision);
        assert_eq!(preview.total_cost, "{0}");
        let selection = preview
            .selection
            .clone()
            .expect("current zero-cost preview returns its payment selection");
        assert_eq!(selection.expected_state_revision, current_revision);
        assert_eq!(selection.mana, Some(PaymentMana::default()));
        let restricted_mana = preview.restricted_mana.clone();
        latest = narset_normal_player_command(
            session,
            directory,
            rows,
            accepted_commands,
            TEKUTHAL_NORMAL_PILOT_ID,
            RuledCommand {
                cmd: Some(Cmd::CommitSpellCast(CommitSpellCast {
                    transaction_id,
                    payment: Some(selection.clone()),
                    restricted_mana,
                })),
            },
            battlefield,
        );
        let engine = session
            .engine
            .as_ref()
            .expect("Narset normal session has an engine after Rebound payment");
        assert!(engine.state.pending_spell_cast.is_none());
        assert!(engine.state.pending_resolution.is_none());
        assert_eq!(engine.state.players[0].mana_pool, mana_before);
        assert!(!engine.state.stack.iter().any(|item| item.id == trigger_id));
        assert_eq!(
            kaya_batch(&latest)
                .events
                .iter()
                .filter(|event| {
                    matches!(&event.ev, Some(Ev::StackResolved(resolved)) if resolved.object_id == trigger_id)
                })
                .count(),
            1,
            "the parent Rebound trigger resolves exactly once at payment commit"
        );
        assert_eq!(
            kaya_batch(&latest)
                .events
                .iter()
                .filter(|event| matches!(&event.ev, Some(Ev::StackPushed(pushed)) if pushed.object_id == object_id))
                .count(),
            1,
            "payment commit publishes the exact Rebound Divination once"
        );
        assert_eq!(engine.state.stack.len(), 1);
        let spell = engine
            .state
            .stack
            .iter()
            .find(|item| item.id == object_id)
            .expect("the exact exiled Divination is now the physical spell");
        assert_eq!(spell.card_id, "divination");
        assert_eq!(spell.cast_by, Some(TEKUTHAL_NORMAL_PILOT_ID));
        assert_eq!(
            spell.cast_method,
            tricerules_core::state::SpellCastMethod::Permission
        );
        assert_eq!(
            spell
                .cast_occurrence
                .expect("the Rebound spell has its exact occurrence")
                .zone_change_generation,
            Some(exile_generation + 1)
        );
        assert_eq!(
            narset_normal_state_field(session, "rebound_spell_grants"),
            serde_json::json!([])
        );
        assert_eq!(
            narset_normal_state_field(session, "scheduled_rebounds"),
            serde_json::json!([])
        );
        println!(
            "Rebound accepted current SpecialCast nonce={} permission={} source={object_id}@{exile_generation} transaction={transaction_id} revision={current_revision} returned_selection={selection:?}",
            choice.resolution_prompt_id,
            permission_id
        );
        latest
    }

    #[allow(clippy::too_many_arguments)]
    fn narset_normal_float_blue(
        session: &mut EngineSession,
        directory: &Path,
        rows: &mut Vec<String>,
        accepted_commands: &mut usize,
        mut latest: IpcResponse,
        battlefield: &mut Vec<BattlefieldObject>,
        player_id: i32,
        count: u32,
    ) -> IpcResponse {
        for expected_blue in 1..=count {
            let (object_id, generation, ability_index, output) =
                kaya_available_mana_abilities(battlefield)
                    .into_iter()
                    .find(|source| {
                        let permanent = battlefield
                            .iter()
                            .find(|permanent| permanent.object_id == source.0)
                            .expect("current mana offer belongs to a battlefield permanent");
                        permanent.card_id == "island" && kaya_output_has_color(&source.3, 'U')
                    })
                    .expect("current untapped Island view offers blue mana");
            latest = narset_normal_player_command(
                session,
                directory,
                rows,
                accepted_commands,
                player_id,
                RuledCommand {
                    cmd: Some(Cmd::ActivateAbility(ActivateAbility {
                        source_object_id: object_id,
                        ability_index,
                        source_zone: AbilitySourceZone::Battlefield as i32,
                        expected_zone_change_generation: generation,
                        ..Default::default()
                    })),
                },
                battlefield,
            );
            kaya_capture_battlefield(&latest, player_id, battlefield);
            let engine = session
                .engine
                .as_ref()
                .expect("Narset normal session remains open after Island mana");
            let player = engine
                .state
                .players
                .iter()
                .find(|player| player.id == player_id)
                .expect("the Island controller remains in the session");
            assert_eq!(player.mana_pool.blue, expected_blue);
            assert_eq!(player.mana_pool.white, 0);
            assert_eq!(player.mana_pool.black, 0);
            assert_eq!(player.mana_pool.red, 0);
            assert_eq!(player.mana_pool.green, 0);
            assert_eq!(player.mana_pool.colorless, 0);
            println!(
                "P{player_id} current Island ObjectId={object_id}@{generation} ability={ability_index} output={output}; pool={:?}",
                player.mana_pool
            );
        }
        latest
    }

    fn narset_normal_cast_opponent_zero_cost(
        session: &mut EngineSession,
        directory: &Path,
        rows: &mut Vec<String>,
        accepted_commands: &mut usize,
        mut latest: IpcResponse,
        battlefield: &mut Vec<BattlefieldObject>,
        expected_object_id: u32,
    ) -> IpcResponse {
        let offer = kaya_batch(&latest)
            .legal_by_player
            .get(&TEKUTHAL_NORMAL_OPPONENT_ID)
            .expect("P29 receives the current legal hand actions")
            .hand_actions
            .iter()
            .find(|offer| {
                offer.kind == HandActionKind::HandActionCastSpell as i32
                    && offer.card_name == "Ornithopter"
            })
            .cloned()
            .expect("the current P29 hand offer includes Ornithopter");
        let (source_object_id, source_generation) = {
            let state = &session
                .engine
                .as_ref()
                .expect("Narset normal session has an engine before Ornithopter")
                .state;
            let p29 = state
                .players
                .iter()
                .find(|player| player.id == TEKUTHAL_NORMAL_OPPONENT_ID)
                .expect("P29 owns the Ornithopter offer");
            let object_id = *p29
                .hand
                .get(usize::try_from(offer.hand_index).expect("hand index fits usize"))
                .expect("current Ornithopter offer resolves to a physical card");
            assert_eq!(object_id, expected_object_id);
            let object = state
                .objects
                .get(&object_id)
                .expect("current Ornithopter offer resolves to a game object");
            assert_eq!(object.zone, tricerules_core::Zone::Hand);
            assert_eq!(
                object.card_id,
                tricerules_cards::registry::global()
                    .id_for_name("Ornithopter")
                    .expect("Ornithopter is in the global registry")
            );
            (
                object_id,
                state
                    .zone_change_generation
                    .get(&object_id)
                    .copied()
                    .unwrap_or(0),
            )
        };
        assert_eq!(source_generation, 0);
        latest = narset_normal_player_command(
            session,
            directory,
            rows,
            accepted_commands,
            TEKUTHAL_NORMAL_OPPONENT_ID,
            RuledCommand {
                cmd: Some(Cmd::BeginSpellCast(BeginSpellCast {
                    announcement: Some(tricerules_proto::ruled::v1::SpellCastAnnouncement {
                        source: Some(CastSource {
                            location: Some(
                                tricerules_proto::ruled::v1::cast_source::Location::HandIndex(
                                    offer.hand_index,
                                ),
                            ),
                            expected_zone_change_generation: None,
                        }),
                        face_index: offer.face_index,
                        cast_cost_group_selections: Vec::new(),
                        cast_method: CastMethod::Normal as i32,
                        ..Default::default()
                    }),
                })),
            },
            battlefield,
        );
        kaya_capture_battlefield(&latest, TEKUTHAL_NORMAL_OPPONENT_ID, battlefield);
        let (transaction_id, current_revision, mana_before) = {
            let engine = session
                .engine
                .as_ref()
                .expect("Narset normal session has an engine after Ornithopter Begin");
            let pending = engine
                .state
                .pending_spell_cast
                .as_ref()
                .expect("current Ornithopter Begin publishes its pending payment");
            assert_eq!(pending.caster, TEKUTHAL_NORMAL_OPPONENT_ID);
            assert_eq!(pending.reserved_object_id, source_object_id);
            let p29 = engine
                .state
                .players
                .iter()
                .find(|player| player.id == TEKUTHAL_NORMAL_OPPONENT_ID)
                .expect("P29 remains in the normal session");
            (
                pending.transaction_id,
                engine.state.command_index,
                p29.mana_pool,
            )
        };
        let payment_request = IpcEnvelope {
            msg: Some(Msg::PaymentQuery(PaymentQuery {
                player_id: TEKUTHAL_NORMAL_OPPONENT_ID,
                preview: Some(PreviewPayment {
                    transaction_id,
                    revision: current_revision,
                    commit_spell_cast: Some(CommitSpellCast {
                        transaction_id,
                        payment: Some(PaymentSelection {
                            expected_state_revision: current_revision,
                            mana: Some(PaymentMana::default()),
                            ..Default::default()
                        }),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
            })),
        };
        let payment_response = session
            .process(&payment_request)
            .expect("P29 Ornithopter PaymentQuery keeps the normal session open");
        pair(directory, rows, &payment_request, &payment_response);
        assert!(
            payment_response.ok,
            "Ornithopter PaymentQuery: {}",
            payment_response.error
        );
        let preview = payment_response
            .batch
            .as_ref()
            .and_then(|batch| batch.payment_preview.as_ref())
            .expect("Ornithopter PaymentQuery returns its current preview");
        assert!(
            preview.valid && preview.complete,
            "Ornithopter preview: {preview:?}"
        );
        assert_eq!(preview.transaction_id, transaction_id);
        assert_eq!(preview.revision, current_revision);
        assert_eq!(preview.total_cost, "{0}");
        let selection = preview
            .selection
            .clone()
            .expect("zero-cost Ornithopter preview returns its selection");
        assert_eq!(selection.expected_state_revision, current_revision);
        assert_eq!(selection.mana, Some(PaymentMana::default()));
        let restricted_mana = preview.restricted_mana.clone();
        latest = narset_normal_player_command(
            session,
            directory,
            rows,
            accepted_commands,
            TEKUTHAL_NORMAL_OPPONENT_ID,
            RuledCommand {
                cmd: Some(Cmd::CommitSpellCast(CommitSpellCast {
                    transaction_id,
                    payment: Some(selection),
                    restricted_mana,
                })),
            },
            battlefield,
        );
        kaya_capture_battlefield(&latest, TEKUTHAL_NORMAL_OPPONENT_ID, battlefield);
        let engine = session
            .engine
            .as_ref()
            .expect("Narset normal session has an engine after Ornithopter payment");
        assert!(engine.state.pending_spell_cast.is_none());
        let p29 = engine
            .state
            .players
            .iter()
            .find(|player| player.id == TEKUTHAL_NORMAL_OPPONENT_ID)
            .expect("P29 remains in the game after Ornithopter payment");
        assert_eq!(p29.mana_pool, mana_before, "zero-cost cast spends no mana");
        assert_eq!(
            p29.mana_pool.blue, 3,
            "the three blue mana remains floating"
        );
        let spell = engine
            .state
            .stack
            .iter()
            .find(|item| item.id == source_object_id)
            .expect("Ornithopter commits as the physical spell");
        assert_eq!(spell.card_id, "ornithopter");
        assert_eq!(spell.cast_by, Some(TEKUTHAL_NORMAL_OPPONENT_ID));
        assert_eq!(
            spell.cast_method,
            tricerules_core::state::SpellCastMethod::Normal
        );
        assert_eq!(
            engine.state.zone_change_generation.get(&source_object_id),
            Some(&1)
        );
        println!(
            "P29 current Ornithopter ObjectId={source_object_id}@{source_generation} paid {transaction_id} with zero mana; floating pool remains {:?}",
            p29.mana_pool
        );
        latest
    }

    #[test]
    fn narset_normal_pilot_records_and_replays_all_three_loyalty_abilities() {
        let player_decks = narset_normal_fixed_decks();
        narset_normal_assert_decks(&player_decks);
        let validation_names = player_decks
            .iter()
            .flat_map(|deck| deck.mainboard_card_name.iter().cloned())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();

        let mut session = EngineSession::new(false);
        let validation = session
            .process(&IpcEnvelope {
                msg: Some(Msg::ValidateDeck(ValidateDeck {
                    card_names: validation_names,
                })),
            })
            .expect("ValidateDeck keeps the Narset normal session open");
        assert!(
            validation.ok,
            "Narset deck validation: {}",
            validation.error
        );
        assert!(validation.missing_card_names.is_empty());

        let root = tempfile::tempdir()
            .expect("unique Narset normal-pilot capture directory")
            .keep();
        println!("retained_capture={}", root.display());
        fs::create_dir(root.join("raw")).expect("create raw capture directory");
        let mut rows = Vec::new();
        let mut manifest = serde_json::json!({
            "format_version": 1,
            "source": "server",
            "privacy": "server_only",
            "capture_id": root.as_path().file_name().unwrap().to_string_lossy().to_string(),
            "engine_build": ENGINE_BUILD,
            "card_data_hash": "",
            "effective_dev_commands_enabled": false,
            "complete": false
        });
        let start = IpcEnvelope {
            msg: Some(Msg::SessionStart(SessionStart {
                game_id: 7_281_102,
                seed: 19,
                player_ids: vec![TEKUTHAL_NORMAL_PILOT_ID, TEKUTHAL_NORMAL_OPPONENT_ID],
                player_decks,
                dev_commands_enabled: false,
                diagnostic_capture_enabled: true,
                ..Default::default()
            })),
        };
        let start_response = session
            .process(&start)
            .expect("normal Narset SessionStart keeps the session open");
        assert!(
            start_response.ok,
            "Narset GameStart: {}",
            start_response.error
        );
        assert!(!start_response.effective_dev_commands_enabled);
        manifest["card_data_hash"] = start_response.card_data_hash.clone().into();
        fs::write(root.as_path().join("manifest.json"), manifest.to_string())
            .expect("write initial normal capture manifest");
        pair(root.as_path(), &mut rows, &start, &start_response);

        let mut battlefield = Vec::new();
        let mut p29_battlefield = Vec::new();
        kaya_capture_battlefield(&start_response, TEKUTHAL_NORMAL_PILOT_ID, &mut battlefield);
        kaya_capture_battlefield(
            &start_response,
            TEKUTHAL_NORMAL_OPPONENT_ID,
            &mut p29_battlefield,
        );
        let mut accepted_commands = 0usize;
        let starting = kaya_starting_player_offer(&start_response)
            .expect("GameStart publishes the current starting-player offer");
        assert!(starting
            .eligible_starting_player_ids
            .contains(&TEKUTHAL_NORMAL_PILOT_ID));
        let mut latest = narset_normal_player_command(
            &mut session,
            root.as_path(),
            &mut rows,
            &mut accepted_commands,
            starting.deciding_player_id,
            RuledCommand {
                cmd: Some(Cmd::ChooseStartingPlayer(ChooseStartingPlayer {
                    starting_player_id: TEKUTHAL_NORMAL_PILOT_ID,
                })),
            },
            &mut battlefield,
        );
        for _ in 0..2 {
            let Some(opening) = kaya_mulligan_offer(&latest) else {
                break;
            };
            latest = narset_normal_player_command(
                &mut session,
                root.as_path(),
                &mut rows,
                &mut accepted_commands,
                opening.deciding_player_id,
                RuledCommand {
                    cmd: Some(Cmd::Mulligan(MulliganDecision { keep: true })),
                },
                &mut battlefield,
            );
        }
        assert!(
            kaya_mulligan_offer(&latest).is_none(),
            "both actual opening hands are kept through current offers"
        );
        let receipt_decks = narset_normal_fixed_decks();
        println!(
            "P17 exact deck order={:?}",
            &receipt_decks[0].mainboard_card_name
        );
        println!(
            "P29 exact deck order={:?}",
            &receipt_decks[1].mainboard_card_name
        );
        println!("frozen normal SessionStart seed=19, game_id=7281102, effective_dev=false");

        let mut p17_islands = 0usize;
        let mut p17_plains = 0usize;
        let mut p17_turns = 0u32;
        for p17_turn in 1..=4 {
            latest = narset_normal_advance_to_main(
                &mut session,
                root.as_path(),
                &mut rows,
                &mut accepted_commands,
                latest,
                &mut battlefield,
                TEKUTHAL_NORMAL_PILOT_ID,
            );
            p17_turns += 1;
            assert_eq!(p17_turns, p17_turn);
            assert!(
                p17_turns <= 24,
                "Narset driver stays inside its P17 turn bound"
            );
            assert_eq!(
                session
                    .engine
                    .as_ref()
                    .expect("Narset normal session has an engine")
                    .state
                    .turn_step,
                tricerules_core::TurnStep::Main1
            );
            latest = narset_normal_play_land(
                &mut session,
                root.as_path(),
                &mut rows,
                &mut accepted_commands,
                &latest,
                &mut battlefield,
                TEKUTHAL_NORMAL_PILOT_ID,
                &mut p17_islands,
                &mut p17_plains,
            );
            if p17_turn < 4 {
                latest = narset_normal_advance_to_main(
                    &mut session,
                    root.as_path(),
                    &mut rows,
                    &mut accepted_commands,
                    latest,
                    &mut p29_battlefield,
                    TEKUTHAL_NORMAL_OPPONENT_ID,
                );
                latest = narset_normal_play_land(
                    &mut session,
                    root.as_path(),
                    &mut rows,
                    &mut accepted_commands,
                    &latest,
                    &mut p29_battlefield,
                    TEKUTHAL_NORMAL_OPPONENT_ID,
                    &mut p17_islands,
                    &mut p17_plains,
                );
            }
        }
        let state = &session
            .engine
            .as_ref()
            .expect("Narset normal session reaches its T4 checkpoint")
            .state;
        assert_eq!(state.turn_step, tricerules_core::TurnStep::Main1);
        assert_eq!(
            state.players[state.active_player_idx].id,
            TEKUTHAL_NORMAL_PILOT_ID
        );
        assert_eq!(state.priority_player_id(), TEKUTHAL_NORMAL_PILOT_ID);
        assert!(state.stack.is_empty());
        assert!(state.pending_spell_cast.is_none());
        assert!(state.pending_resolution.is_none());
        assert_eq!((p17_islands, p17_plains), (3, 1));
        assert_eq!(
            battlefield
                .iter()
                .filter(|permanent| permanent.card_id == "island")
                .count(),
            3,
            "P17 has three ordinary Islands at T4"
        );
        assert_eq!(
            battlefield
                .iter()
                .filter(|permanent| permanent.card_id == "plains")
                .count(),
            1,
            "P17 has one ordinary Plains at T4"
        );

        narset_normal_cast_from_current_offer(
            &mut session,
            root.as_path(),
            &mut rows,
            &mut accepted_commands,
            latest,
            &mut battlefield,
            "Narset Transcendent",
            "{2}{W}{U}",
            3,
            1,
            Some(30),
            true,
        );
        let narset = battlefield
            .iter()
            .find(|permanent| permanent.card_id == "narset_transcendent")
            .expect("the ordinary T4 cast resolves as the actual Narset permanent");
        assert_eq!((narset.object_id, narset.zone_change_generation), (30, 3));
        assert_eq!(narset.loyalty, 6);
        assert!(session
            .engine
            .as_ref()
            .expect("Narset session remains open after the printed cast")
            .state
            .stack
            .is_empty());

        let plus_one = narset
            .activated_abilities
            .iter()
            .find(|ability| {
                ability.presentation.as_ref().is_some_and(|presentation| {
                    presentation.path.iter().any(|part| {
                        part.kind == PresentationPathKind::Ability as i32
                            && part.id == "activated_01"
                    })
                })
            })
            .expect("current Narset view identifies the +1 by stable ability ID");
        assert!(plus_one.activatable, "current Narset view offers its +1");
        let narset_object_id = narset.object_id;
        let narset_generation = narset.zone_change_generation;
        let ability_index = plus_one.ability_index;
        let (hand_before, library_before, draws_before) = {
            let state = &session.engine.as_ref().unwrap().state;
            let player = state
                .players
                .iter()
                .find(|player| player.id == TEKUTHAL_NORMAL_PILOT_ID)
                .expect("P17 remains in the normal session");
            (
                player.hand.len(),
                player.library.len(),
                state
                    .turn_history
                    .current
                    .player(TEKUTHAL_NORMAL_PILOT_ID)
                    .cards_drawn,
            )
        };
        latest = narset_normal_player_command(
            &mut session,
            root.as_path(),
            &mut rows,
            &mut accepted_commands,
            TEKUTHAL_NORMAL_PILOT_ID,
            RuledCommand {
                cmd: Some(Cmd::ActivateAbility(ActivateAbility {
                    source_object_id: narset_object_id,
                    ability_index,
                    source_zone: AbilitySourceZone::Battlefield as i32,
                    expected_zone_change_generation: narset_generation,
                    ..Default::default()
                })),
            },
            &mut battlefield,
        );
        let activated_narset = battlefield
            .iter()
            .find(|permanent| permanent.card_id == "narset_transcendent")
            .expect("current response retains the activated Narset permanent");
        assert_eq!(activated_narset.loyalty, 7, "the ordinary +1 cost is paid");
        assert_eq!(
            session
                .engine
                .as_ref()
                .expect("Narset activation has an engine")
                .state
                .stack
                .last()
                .and_then(|item| item.source_permanent_id),
            Some(narset_object_id)
        );

        let mut choice = kaya_choice(&latest, ChoiceKind::LibraryLook);
        for _ in 0..8 {
            if choice.is_some() {
                break;
            }
            let priority_player = session
                .engine
                .as_ref()
                .expect("Narset ability has an engine while resolving")
                .state
                .priority_player_id();
            latest = narset_normal_player_command(
                &mut session,
                root.as_path(),
                &mut rows,
                &mut accepted_commands,
                priority_player,
                RuledCommand {
                    cmd: Some(Cmd::PassPriority(PassPriority {})),
                },
                &mut battlefield,
            );
            choice = kaya_choice(&latest, ChoiceKind::LibraryLook);
        }
        let choice = choice.expect("Narset's resolved +1 publishes its private top-card offer");
        assert_eq!(choice.deciding_player_id, TEKUTHAL_NORMAL_PILOT_ID);
        assert_ne!(
            choice.resolution_prompt_id, 0,
            "current choice nonce is present"
        );
        assert_eq!(choice.min, 0, "Narset may decline the top card");
        assert_eq!(choice.max, 1);
        assert_eq!(choice.candidate_object_ids, vec![34]);
        assert_eq!(choice.candidate_card_ids, vec!["divination".to_string()]);
        assert_eq!(choice.candidate_names, vec!["Divination".to_string()]);
        assert_eq!(choice.candidate_selectable, vec![true]);
        assert!(
            kaya_batch(&latest)
                .events
                .iter()
                .all(|event| !matches!(event.ev, Some(Ev::CardsRevealed(_)))),
            "looking privately at the offered top card has not revealed it publicly"
        );

        latest = narset_normal_player_command(
            &mut session,
            root.as_path(),
            &mut rows,
            &mut accepted_commands,
            choice.deciding_player_id,
            RuledCommand {
                cmd: Some(Cmd::SubmitResolutionChoice(SubmitResolutionChoice {
                    resolution_prompt_id: choice.resolution_prompt_id,
                    chosen_object_ids: vec![34],
                    decision: ResolutionChoiceDecision::Unspecified as i32,
                    ..Default::default()
                })),
            },
            &mut battlefield,
        );
        let revealed = kaya_batch(&latest)
            .events
            .iter()
            .filter_map(|event| match event.ev.as_ref() {
                Some(Ev::CardsRevealed(cards)) => Some(cards),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            revealed.len(),
            1,
            "accepting one card emits one reveal group"
        );
        assert_eq!(revealed[0].zone_owner_player_id, TEKUTHAL_NORMAL_PILOT_ID);
        assert_eq!(
            revealed[0].cards.len(),
            1,
            "only the selected top card is revealed"
        );
        assert_eq!(revealed[0].cards[0].object_id, 34);
        assert_eq!(revealed[0].cards[0].zone_change_generation, 0);
        assert_eq!(revealed[0].cards[0].card_id, "divination");
        assert_eq!(revealed[0].cards[0].card_name, "Divination");
        assert!(kaya_choice(&latest, ChoiceKind::LibraryLook).is_none());

        let state = &session
            .engine
            .as_ref()
            .expect("Narset choice response retains the normal session")
            .state;
        let p17 = state
            .players
            .iter()
            .find(|player| player.id == TEKUTHAL_NORMAL_PILOT_ID)
            .expect("P17 remains in the session");
        assert!(
            p17.hand.contains(&34),
            "the selected Divination enters P17's hand"
        );
        assert_eq!(state.objects[&34].zone, tricerules_core::Zone::Hand);
        assert_eq!(state.zone_change_generation.get(&34), Some(&1));
        assert_eq!(p17.hand.len(), hand_before + 1);
        assert_eq!(p17.library.len(), library_before - 1);
        assert_eq!(
            state
                .turn_history
                .current
                .player(TEKUTHAL_NORMAL_PILOT_ID)
                .cards_drawn,
            draws_before,
            "Narset's +1 moves the selected card to hand without drawing"
        );
        assert!(state.stack.is_empty());
        assert!(state.pending_resolution.is_none());
        let final_narset = battlefield
            .iter()
            .find(|permanent| permanent.card_id == "narset_transcendent")
            .expect("Narset remains on the battlefield after its +1 resolves");
        assert_eq!(final_narset.loyalty, 7);

        // Finish P29's ordinary T4 before beginning the fixed T5 Rebound line.
        latest = narset_normal_advance_to_main(
            &mut session,
            root.as_path(),
            &mut rows,
            &mut accepted_commands,
            latest,
            &mut p29_battlefield,
            TEKUTHAL_NORMAL_OPPONENT_ID,
        );
        latest = narset_normal_play_land(
            &mut session,
            root.as_path(),
            &mut rows,
            &mut accepted_commands,
            &latest,
            &mut p29_battlefield,
            TEKUTHAL_NORMAL_OPPONENT_ID,
            &mut p17_islands,
            &mut p17_plains,
        );
        latest = narset_normal_advance_to_main(
            &mut session,
            root.as_path(),
            &mut rows,
            &mut accepted_commands,
            latest,
            &mut battlefield,
            TEKUTHAL_NORMAL_PILOT_ID,
        );
        p17_turns += 1;
        assert_eq!(p17_turns, 5);
        assert!(
            p17_turns <= 24,
            "Narset driver stays inside its P17 turn bound"
        );
        assert_eq!(
            session.engine.as_ref().unwrap().state.active_player_id(),
            TEKUTHAL_NORMAL_PILOT_ID,
            "the fixed driver reaches P17's natural fifth turn"
        );
        narset_normal_play_land(
            &mut session,
            root.as_path(),
            &mut rows,
            &mut accepted_commands,
            &latest,
            &mut battlefield,
            TEKUTHAL_NORMAL_PILOT_ID,
            &mut p17_islands,
            &mut p17_plains,
        );
        let narset_before_minus_two = battlefield
            .iter()
            .find(|permanent| permanent.card_id == "narset_transcendent")
            .expect("the actual Narset remains in P17's current battlefield view");
        assert_eq!(narset_before_minus_two.loyalty, 7);
        let narset_object_id = narset_before_minus_two.object_id;
        let narset_generation = narset_before_minus_two.zone_change_generation;
        latest = narset_normal_activate_loyalty(
            &mut session,
            root.as_path(),
            &mut rows,
            &mut accepted_commands,
            &mut battlefield,
            "activated_02",
        );
        {
            let state = &session.engine.as_ref().unwrap().state;
            assert_eq!(state.turn_step, tricerules_core::TurnStep::Main1);
            assert_eq!(state.active_player_id(), TEKUTHAL_NORMAL_PILOT_ID);
            assert_eq!(state.priority_player_id(), TEKUTHAL_NORMAL_PILOT_ID);
            assert_eq!(
                state.objects[&narset_object_id]
                    .counter_count(tricerules_cards::primitives::CounterKind::Loyalty),
                5
            );
            assert_eq!(state.stack.len(), 1);
            assert_eq!(state.stack[0].source_permanent_id, Some(narset_object_id));
            assert_eq!(state.stack[0].source_zone_change, narset_generation);
        }
        latest = narset_normal_resolve_top_stack(
            &mut session,
            root.as_path(),
            &mut rows,
            &mut accepted_commands,
            latest,
            &mut battlefield,
        );
        {
            let state = &session.engine.as_ref().unwrap().state;
            assert!(state.stack.is_empty());
            assert!(state.pending_resolution.is_none());
            assert_eq!(state.active_event_observers.len(), 1);
            assert_eq!(
                narset_normal_state_field(&session, "rebound_spell_grants"),
                serde_json::json!([])
            );
            assert_eq!(
                narset_normal_state_field(&session, "scheduled_rebounds"),
                serde_json::json!([])
            );
        }

        let original_divination = 36;
        {
            let state = &session.engine.as_ref().unwrap().state;
            assert!(state.players[0].hand.contains(&original_divination));
            assert_eq!(
                state.objects[&original_divination].zone,
                tricerules_core::Zone::Hand
            );
            assert_eq!(
                state
                    .zone_change_generation
                    .get(&original_divination)
                    .copied()
                    .unwrap_or(0),
                0
            );
        }
        latest = narset_normal_cast_from_current_offer(
            &mut session,
            root.as_path(),
            &mut rows,
            &mut accepted_commands,
            latest,
            &mut battlefield,
            "Divination",
            "{2}{U}",
            3,
            0,
            Some(original_divination),
            false,
        );
        let (original_occurrence, original_stack_generation, grant_trigger_id) = {
            let state = &session.engine.as_ref().unwrap().state;
            assert_eq!(
                state.objects[&original_divination].zone,
                tricerules_core::Zone::Stack
            );
            let spell = state
                .stack
                .iter()
                .find(|item| item.id == original_divination)
                .expect("the exact original hand Divination remains on the stack");
            assert_eq!(spell.cast_by, Some(TEKUTHAL_NORMAL_PILOT_ID));
            assert_eq!(
                spell.cast_method,
                tricerules_core::state::SpellCastMethod::Normal
            );
            let occurrence = spell
                .cast_occurrence
                .expect("the original Divination has its exact cast occurrence");
            let spell_index = state
                .stack
                .iter()
                .position(|item| item.id == original_divination)
                .expect("the original Divination has a stack position");
            let triggers = state
                .stack
                .iter()
                .filter(|item| item.is_triggered)
                .collect::<Vec<_>>();
            assert_eq!(
                triggers.len(),
                1,
                "the actual cast stages one ordinary grant trigger"
            );
            let trigger = triggers[0];
            let trigger_index = state
                .stack
                .iter()
                .position(|item| item.id == trigger.id)
                .expect("the grant trigger has a current stack position");
            assert!(
                trigger_index > spell_index,
                "the grant trigger is above the exact spell"
            );
            assert_eq!(trigger.controller, TEKUTHAL_NORMAL_PILOT_ID);
            assert_eq!(trigger.source_permanent_id, Some(narset_object_id));
            assert_eq!(trigger.source_zone_change, narset_generation);
            let trigger_json = serde_json::to_value(trigger).expect("serialize grant trigger");
            assert_eq!(
                trigger_json["trigger_context"]["triggering_spell"],
                serde_json::to_value(Some(occurrence)).expect("serialize exact spell occurrence")
            );
            assert_eq!(
                kaya_batch(&latest)
                    .events
                    .iter()
                    .filter(|event| matches!(event.ev, Some(Ev::StackPushed(_))))
                    .count(),
                2,
                "the cast response publishes the physical spell and its grant trigger"
            );
            assert!(state.active_event_observers.is_empty());
            assert_eq!(
                narset_normal_state_field(&session, "rebound_spell_grants"),
                serde_json::json!([]),
                "the grant does not exist before its trigger resolves"
            );
            assert_eq!(
                narset_normal_state_field(&session, "scheduled_rebounds"),
                serde_json::json!([]),
                "no Rebound is scheduled before the grant trigger resolves"
            );
            let generation = state
                .zone_change_generation
                .get(&original_divination)
                .copied()
                .expect("the original Divination on the stack has a generation");
            assert_eq!(occurrence.object_id, original_divination);
            assert_eq!(occurrence.zone_change_generation, Some(generation));
            (occurrence, generation, trigger.id)
        };

        latest = narset_normal_resolve_top_stack(
            &mut session,
            root.as_path(),
            &mut rows,
            &mut accepted_commands,
            latest,
            &mut battlefield,
        );
        {
            let state = &session.engine.as_ref().unwrap().state;
            assert_eq!(state.stack.len(), 1, "only the actual Divination remains");
            assert_eq!(state.stack[0].id, original_divination);
            assert!(kaya_batch(&latest).events.iter().any(|event| {
                matches!(event.ev, Some(Ev::StackResolved(resolved)) if resolved.object_id == grant_trigger_id)
            }));
            assert_eq!(
                narset_normal_state_field(&session, "rebound_spell_grants"),
                serde_json::json!([[
                    original_occurrence.object_id,
                    original_occurrence
                        .zone_change_generation
                        .map(|generation| generation.to_string())
                ]]),
                "the resolving trigger grants Rebound to this exact cast occurrence"
            );
            assert_eq!(
                narset_normal_state_field(&session, "scheduled_rebounds"),
                serde_json::json!([])
            );
        }
        let (
            hand_before_original_resolution,
            library_before_original_resolution,
            draws_before_original_resolution,
        ) = {
            let state = &session.engine.as_ref().unwrap().state;
            let p17 = state
                .players
                .iter()
                .find(|player| player.id == TEKUTHAL_NORMAL_PILOT_ID)
                .expect("P17 remains in the Rebound cast turn");
            (
                p17.hand.len(),
                p17.library.len(),
                state
                    .turn_history
                    .current
                    .player(TEKUTHAL_NORMAL_PILOT_ID)
                    .cards_drawn,
            )
        };
        latest = narset_normal_resolve_top_stack(
            &mut session,
            root.as_path(),
            &mut rows,
            &mut accepted_commands,
            latest,
            &mut battlefield,
        );
        let (original_exile_generation, saved_rebound_schedule) = {
            let state = &session.engine.as_ref().unwrap().state;
            let p17 = state
                .players
                .iter()
                .find(|player| player.id == TEKUTHAL_NORMAL_PILOT_ID)
                .expect("P17 remains in the Rebound resolution");
            let resolved = kaya_batch(&latest)
                .events
                .iter()
                .find_map(|event| match event.ev.as_ref() {
                    Some(Ev::StackResolved(resolved))
                        if resolved.object_id == original_divination =>
                    {
                        Some(resolved)
                    }
                    _ => None,
                })
                .expect("the original Divination resolves exactly once");
            assert_eq!(
                resolved.destination,
                tricerules_proto::ruled::v1::StackResolveDestination::Exile as i32
            );
            assert!(state.stack.is_empty());
            assert!(state.pending_resolution.is_none());
            assert_eq!(p17.hand.len(), hand_before_original_resolution + 2);
            assert_eq!(p17.library.len() + 2, library_before_original_resolution);
            assert_eq!(
                state
                    .turn_history
                    .current
                    .player(TEKUTHAL_NORMAL_PILOT_ID)
                    .cards_drawn,
                draws_before_original_resolution + 2,
                "Divination draws two immediately before its own resolution"
            );
            assert_eq!(
                state.objects[&original_divination].zone,
                tricerules_core::Zone::Exile
            );
            let generation = state
                .zone_change_generation
                .get(&original_divination)
                .copied()
                .expect("the same physical Divination has an Exile generation");
            assert_eq!(generation, original_stack_generation + 1);
            let grants = narset_normal_state_field(&session, "rebound_spell_grants");
            assert_eq!(grants, serde_json::json!([]));
            let schedule = narset_normal_state_field(&session, "scheduled_rebounds");
            let records = schedule
                .as_array()
                .expect("diagnostics expose scheduled Rebound records");
            assert_eq!(records.len(), 1);
            let record = &records[0];
            assert_eq!(
                record["source"]["object_id"],
                serde_json::json!(original_divination)
            );
            assert_eq!(
                record["source"]["zone_change_generation"],
                serde_json::json!(original_stack_generation.to_string())
            );
            assert_eq!(
                record["exiled"]["object_id"],
                serde_json::json!(original_divination)
            );
            assert_eq!(
                record["exiled"]["zone_change_generation"],
                serde_json::json!(generation.to_string())
            );
            assert_eq!(
                record["controller"],
                serde_json::json!(TEKUTHAL_NORMAL_PILOT_ID)
            );
            assert_eq!(record["card_id"], serde_json::json!("divination"));
            assert_eq!(record["card_name"], serde_json::json!("Divination"));
            assert_eq!(record["source_face_index"], serde_json::json!("0"));
            (generation, schedule)
        };
        assert_eq!(
            session
                .engine
                .as_ref()
                .unwrap()
                .state
                .zone_change_generation[&original_divination],
            original_exile_generation
        );
        assert_eq!(
            session.engine.as_ref().unwrap().state.stack.len(),
            0,
            "the exact Rebound schedule waits for its controller's upkeep"
        );

        latest = narset_normal_advance_to_step(
            &mut session,
            root.as_path(),
            &mut rows,
            &mut accepted_commands,
            latest,
            &mut p29_battlefield,
            TEKUTHAL_NORMAL_OPPONENT_ID,
            tricerules_core::TurnStep::Upkeep,
        );
        {
            let state = &session.engine.as_ref().unwrap().state;
            assert_eq!(state.turn_step, tricerules_core::TurnStep::Upkeep);
            assert_eq!(state.active_player_id(), TEKUTHAL_NORMAL_OPPONENT_ID);
            assert_eq!(state.priority_player_id(), TEKUTHAL_NORMAL_OPPONENT_ID);
            assert!(state.stack.is_empty());
            assert_eq!(
                narset_normal_state_field(&session, "scheduled_rebounds"),
                saved_rebound_schedule
            );
            assert!(state
                .active_exile_play_permissions
                .iter()
                .all(|permission| { permission.object_id != original_divination }));
            let p17_offers = kaya_batch(&latest)
                .legal_by_player
                .get(&TEKUTHAL_NORMAL_PILOT_ID)
                .expect("P17 retains a current private offer view during P29 upkeep");
            assert!(p17_offers
                .zone_cast_actions
                .iter()
                .all(|action| action.object_id != original_divination));
            assert!(state.stack.iter().all(|item| !item.is_triggered));
        }
        latest = narset_normal_advance_to_main(
            &mut session,
            root.as_path(),
            &mut rows,
            &mut accepted_commands,
            latest,
            &mut p29_battlefield,
            TEKUTHAL_NORMAL_OPPONENT_ID,
        );
        latest = narset_normal_play_land(
            &mut session,
            root.as_path(),
            &mut rows,
            &mut accepted_commands,
            &latest,
            &mut p29_battlefield,
            TEKUTHAL_NORMAL_OPPONENT_ID,
            &mut p17_islands,
            &mut p17_plains,
        );
        {
            let state = &session.engine.as_ref().unwrap().state;
            let p29 = state
                .players
                .iter()
                .find(|player| player.id == TEKUTHAL_NORMAL_OPPONENT_ID)
                .expect("P29 remains in the normal game");
            assert!(p29
                .hand
                .iter()
                .any(|object_id| state.objects[object_id].card_id == "divination"));
            assert!(p29
                .hand
                .iter()
                .any(|object_id| state.objects[object_id].card_id == "ornithopter"));
        }

        latest = narset_normal_advance_to_step(
            &mut session,
            root.as_path(),
            &mut rows,
            &mut accepted_commands,
            latest,
            &mut battlefield,
            TEKUTHAL_NORMAL_PILOT_ID,
            tricerules_core::TurnStep::Upkeep,
        );
        let rebound_trigger_id = {
            let state = &session.engine.as_ref().unwrap().state;
            assert_eq!(state.turn_step, tricerules_core::TurnStep::Upkeep);
            assert_eq!(state.active_player_id(), TEKUTHAL_NORMAL_PILOT_ID);
            assert_eq!(state.priority_player_id(), TEKUTHAL_NORMAL_PILOT_ID);
            assert!(state.stack.is_empty());
            let choice = kaya_choice(&latest, ChoiceKind::SpecialCast)
                .expect("P17 T6 upkeep resolves the actual Rebound trigger into its current offer");
            assert_eq!(choice.candidate_object_ids, vec![original_divination]);
            let pending = state
                .pending_resolution
                .as_ref()
                .expect("the current Rebound SpecialCast choice remains pending");
            let tricerules_core::state::ResolutionContinuation::SpecialCast {
                stack, exiled, ..
            } = &pending.continuation
            else {
                panic!("P17 upkeep choice belongs to the Rebound SpecialCast continuation");
            };
            assert!(stack.item.is_triggered);
            assert_eq!(stack.item.card_id, "divination");
            assert_eq!(exiled.object_id, original_divination);
            assert_eq!(exiled.zone_change_generation, original_exile_generation);
            assert_eq!(
                narset_normal_state_field(&session, "scheduled_rebounds"),
                serde_json::json!([])
            );
            stack.item.id
        };
        latest = narset_normal_accept_rebound_offer(
            &mut session,
            root.as_path(),
            &mut rows,
            &mut accepted_commands,
            latest,
            &mut battlefield,
            original_divination,
            original_exile_generation,
        );
        let (hand_before_rebound_resolution, draws_before_rebound_resolution) = {
            let state = &session.engine.as_ref().unwrap().state;
            let p17 = state
                .players
                .iter()
                .find(|player| player.id == TEKUTHAL_NORMAL_PILOT_ID)
                .expect("P17 remains in the Rebound turn");
            (
                p17.hand.len(),
                state
                    .turn_history
                    .current
                    .player(TEKUTHAL_NORMAL_PILOT_ID)
                    .cards_drawn,
            )
        };
        latest = narset_normal_resolve_top_stack(
            &mut session,
            root.as_path(),
            &mut rows,
            &mut accepted_commands,
            latest,
            &mut battlefield,
        );
        {
            let state = &session.engine.as_ref().unwrap().state;
            let p17 = state
                .players
                .iter()
                .find(|player| player.id == TEKUTHAL_NORMAL_PILOT_ID)
                .expect("P17 remains after the free Rebound spell");
            let rebound_resolution = kaya_batch(&latest)
                .events
                .iter()
                .find_map(|event| match event.ev.as_ref() {
                    Some(Ev::StackResolved(resolved))
                        if resolved.object_id == original_divination =>
                    {
                        Some(resolved)
                    }
                    _ => None,
                })
                .expect("the accepted Rebound Divination resolves");
            assert_eq!(
                rebound_resolution.destination,
                tricerules_proto::ruled::v1::StackResolveDestination::Graveyard as i32
            );
            assert_eq!(p17.hand.len(), hand_before_rebound_resolution + 2);
            assert_eq!(
                state
                    .turn_history
                    .current
                    .player(TEKUTHAL_NORMAL_PILOT_ID)
                    .cards_drawn,
                draws_before_rebound_resolution + 2
            );
            assert_eq!(
                state.objects[&original_divination].zone,
                tricerules_core::Zone::Graveyard
            );
            assert_eq!(
                state.zone_change_generation.get(&original_divination),
                Some(&(original_exile_generation + 2))
            );
            assert!(state.stack.is_empty());
            assert!(state.pending_resolution.is_none());
            assert!(state
                .active_exile_play_permissions
                .iter()
                .all(|permission| { permission.object_id != original_divination }));
            assert_eq!(
                narset_normal_state_field(&session, "rebound_spell_grants"),
                serde_json::json!([])
            );
            assert_eq!(
                narset_normal_state_field(&session, "scheduled_rebounds"),
                serde_json::json!([]),
                "the Exile cast does not create a Rebound loop"
            );
            assert_eq!(
                kaya_batch(&latest)
                    .events
                    .iter()
                    .filter(|event| {
                        matches!(&event.ev, Some(Ev::StackResolved(resolved)) if resolved.object_id == rebound_trigger_id)
                    })
                    .count(),
                0,
                "the parent Rebound trigger does not resolve again with its child Divination"
            );
        }
        latest = narset_normal_advance_to_main(
            &mut session,
            root.as_path(),
            &mut rows,
            &mut accepted_commands,
            latest,
            &mut battlefield,
            TEKUTHAL_NORMAL_PILOT_ID,
        );
        p17_turns += 1;
        assert_eq!(p17_turns, 6);
        assert!(
            p17_turns <= 24,
            "Narset driver stays inside its P17 turn bound"
        );
        latest = narset_normal_play_land(
            &mut session,
            root.as_path(),
            &mut rows,
            &mut accepted_commands,
            &latest,
            &mut battlefield,
            TEKUTHAL_NORMAL_PILOT_ID,
            &mut p17_islands,
            &mut p17_plains,
        );

        for p17_turn in 6..=9 {
            assert_eq!(p17_turns, p17_turn);
            assert!(
                p17_turns <= 24,
                "Narset driver stays inside its P17 turn bound"
            );
            let narset = battlefield
                .iter()
                .find(|permanent| permanent.card_id == "narset_transcendent")
                .expect("Narset remains present through the four later +1 abilities");
            let loyalty_before = narset.loyalty;
            let expected_loyalty = p17_turn;
            assert_eq!(loyalty_before, expected_loyalty - 1);
            let (hand_before, library_before, draws_before) = {
                let state = &session.engine.as_ref().unwrap().state;
                let p17 = state
                    .players
                    .iter()
                    .find(|player| player.id == TEKUTHAL_NORMAL_PILOT_ID)
                    .expect("P17 remains in the normal session");
                (
                    p17.hand.len(),
                    p17.library.len(),
                    state
                        .turn_history
                        .current
                        .player(TEKUTHAL_NORMAL_PILOT_ID)
                        .cards_drawn,
                )
            };
            latest = narset_normal_activate_loyalty(
                &mut session,
                root.as_path(),
                &mut rows,
                &mut accepted_commands,
                &mut battlefield,
                "activated_01",
            );
            assert_eq!(
                battlefield
                    .iter()
                    .find(|permanent| permanent.card_id == "narset_transcendent")
                    .expect("the +1 source remains on the battlefield")
                    .loyalty,
                loyalty_before + 1
            );
            latest = narset_normal_resolve_top_stack(
                &mut session,
                root.as_path(),
                &mut rows,
                &mut accepted_commands,
                latest,
                &mut battlefield,
            );
            let look = kaya_choice(&latest, ChoiceKind::LibraryLook)
                .expect("each later +1 resolves to its current optional look");
            assert_eq!(look.min, 0);
            assert!(look.max <= 1);
            if look.max == 0 {
                assert!(look
                    .candidate_selectable
                    .iter()
                    .all(|selectable| !selectable));
            }
            latest = narset_normal_decline_current_look(
                &mut session,
                root.as_path(),
                &mut rows,
                &mut accepted_commands,
                latest,
                &mut battlefield,
            );
            {
                let state = &session.engine.as_ref().unwrap().state;
                let p17 = state
                    .players
                    .iter()
                    .find(|player| player.id == TEKUTHAL_NORMAL_PILOT_ID)
                    .expect("P17 remains after declining the private look");
                assert_eq!(p17.hand.len(), hand_before);
                assert_eq!(p17.library.len(), library_before);
                assert_eq!(
                    state
                        .turn_history
                        .current
                        .player(TEKUTHAL_NORMAL_PILOT_ID)
                        .cards_drawn,
                    draws_before
                );
                assert_eq!(state.objects[&30].zone, tricerules_core::Zone::Battlefield);
                assert_eq!(
                    state.objects[&30]
                        .counter_count(tricerules_cards::primitives::CounterKind::Loyalty),
                    expected_loyalty
                );
                assert!(state.stack.is_empty());
                assert!(state.pending_resolution.is_none());
            }
            println!("P17 T{p17_turn} resolved activated_01 and declined the current LibraryLook offer with an empty answer");

            if p17_turn < 9 {
                latest = narset_normal_advance_to_main(
                    &mut session,
                    root.as_path(),
                    &mut rows,
                    &mut accepted_commands,
                    latest,
                    &mut p29_battlefield,
                    TEKUTHAL_NORMAL_OPPONENT_ID,
                );
                latest = narset_normal_play_land(
                    &mut session,
                    root.as_path(),
                    &mut rows,
                    &mut accepted_commands,
                    &latest,
                    &mut p29_battlefield,
                    TEKUTHAL_NORMAL_OPPONENT_ID,
                    &mut p17_islands,
                    &mut p17_plains,
                );
                latest = narset_normal_advance_to_main(
                    &mut session,
                    root.as_path(),
                    &mut rows,
                    &mut accepted_commands,
                    latest,
                    &mut battlefield,
                    TEKUTHAL_NORMAL_PILOT_ID,
                );
                p17_turns += 1;
                assert_eq!(p17_turns, p17_turn + 1);
                assert!(
                    p17_turns <= 24,
                    "Narset driver stays inside its P17 turn bound"
                );
                assert_eq!(
                    session.engine.as_ref().unwrap().state.active_player_id(),
                    TEKUTHAL_NORMAL_PILOT_ID
                );
                latest = narset_normal_play_land(
                    &mut session,
                    root.as_path(),
                    &mut rows,
                    &mut accepted_commands,
                    &latest,
                    &mut battlefield,
                    TEKUTHAL_NORMAL_PILOT_ID,
                    &mut p17_islands,
                    &mut p17_plains,
                );
            }
        }

        // P29's pre-emblem T9 offer proves Divination remains castable before the restriction.
        latest = narset_normal_advance_to_main(
            &mut session,
            root.as_path(),
            &mut rows,
            &mut accepted_commands,
            latest,
            &mut p29_battlefield,
            TEKUTHAL_NORMAL_OPPONENT_ID,
        );
        latest = narset_normal_play_land(
            &mut session,
            root.as_path(),
            &mut rows,
            &mut accepted_commands,
            &latest,
            &mut p29_battlefield,
            TEKUTHAL_NORMAL_OPPONENT_ID,
            &mut p17_islands,
            &mut p17_plains,
        );
        {
            let state = &session.engine.as_ref().unwrap().state;
            assert_eq!(state.turn_step, tricerules_core::TurnStep::Main1);
            assert_eq!(state.active_player_id(), TEKUTHAL_NORMAL_OPPONENT_ID);
            assert_eq!(state.priority_player_id(), TEKUTHAL_NORMAL_OPPONENT_ID);
            assert!(state.stack.is_empty());
            assert!(state.pending_resolution.is_none());
            assert!(state.static_emblems.is_empty());
            assert!(
                p29_battlefield
                    .iter()
                    .filter(|permanent| { permanent.card_id == "island" && !permanent.tapped })
                    .count()
                    >= 3
            );
            let p29 = state
                .players
                .iter()
                .find(|player| player.id == TEKUTHAL_NORMAL_OPPONENT_ID)
                .expect("P29 remains the active player before the emblem");
            assert!(p29
                .hand
                .iter()
                .any(|object_id| state.objects[object_id].card_id == "divination"));
            assert!(p29
                .hand
                .iter()
                .any(|object_id| state.objects[object_id].card_id == "ornithopter"));
            let actions = kaya_batch(&latest)
                .legal_by_player
                .get(&TEKUTHAL_NORMAL_OPPONENT_ID)
                .expect("P29 receives its current pre-emblem cast offers")
                .hand_actions
                .iter()
                .filter(|offer| offer.kind == HandActionKind::HandActionCastSpell as i32)
                .collect::<Vec<_>>();
            assert!(actions.iter().any(|offer| offer.card_name == "Divination"));
            assert!(actions.iter().any(|offer| offer.card_name == "Ornithopter"));
        }

        latest = narset_normal_advance_to_main(
            &mut session,
            root.as_path(),
            &mut rows,
            &mut accepted_commands,
            latest,
            &mut battlefield,
            TEKUTHAL_NORMAL_PILOT_ID,
        );
        p17_turns += 1;
        assert_eq!(p17_turns, 10);
        assert!(
            p17_turns <= 24,
            "Narset driver stays inside its P17 turn bound"
        );
        assert_eq!(
            session.engine.as_ref().unwrap().state.active_player_id(),
            TEKUTHAL_NORMAL_PILOT_ID
        );
        narset_normal_play_land(
            &mut session,
            root.as_path(),
            &mut rows,
            &mut accepted_commands,
            &latest,
            &mut battlefield,
            TEKUTHAL_NORMAL_PILOT_ID,
            &mut p17_islands,
            &mut p17_plains,
        );
        let narset_before_ultimate = battlefield
            .iter()
            .find(|permanent| permanent.card_id == "narset_transcendent")
            .expect("the current P17 T10 battlefield view has Narset");
        assert_eq!(narset_before_ultimate.loyalty, 9);
        let ultimate_source_generation = narset_before_ultimate.zone_change_generation;
        latest = narset_normal_activate_loyalty(
            &mut session,
            root.as_path(),
            &mut rows,
            &mut accepted_commands,
            &mut battlefield,
            "activated_03",
        );
        {
            let state = &session.engine.as_ref().unwrap().state;
            assert_eq!(state.objects[&30].zone, tricerules_core::Zone::Graveyard);
            assert_eq!(
                state.zone_change_generation.get(&30),
                Some(&(ultimate_source_generation + 1)),
                "paying -9 moves Narset to the Graveyard before its ability resolves"
            );
            assert_eq!(state.stack.len(), 1);
            assert_eq!(state.stack[0].source_permanent_id, Some(30));
            assert_eq!(
                state.stack[0].source_zone_change,
                ultimate_source_generation
            );
            assert!(state.static_emblems.is_empty());
        }
        latest = narset_normal_resolve_top_stack(
            &mut session,
            root.as_path(),
            &mut rows,
            &mut accepted_commands,
            latest,
            &mut battlefield,
        );
        {
            let state = &session.engine.as_ref().unwrap().state;
            assert!(state.stack.is_empty());
            assert_eq!(state.static_emblems.len(), 1);
            let emblem = &state.static_emblems[0];
            assert_eq!(emblem.controller, TEKUTHAL_NORMAL_PILOT_ID);
            assert_eq!(emblem.emblem_id, "narset_transcendent_emblem");
            assert!(emblem.effects.iter().any(|effect| matches!(
                effect,
                tricerules_cards::primitives::StaticEmblemEffect::OpponentsCantCastNoncreatureSpells
            )));
            assert_eq!(state.objects[&30].zone, tricerules_core::Zone::Graveyard);
        }

        latest = narset_normal_advance_to_main(
            &mut session,
            root.as_path(),
            &mut rows,
            &mut accepted_commands,
            latest,
            &mut p29_battlefield,
            TEKUTHAL_NORMAL_OPPONENT_ID,
        );
        {
            let state = &session.engine.as_ref().unwrap().state;
            assert_eq!(state.turn_step, tricerules_core::TurnStep::Main1);
            assert_eq!(state.active_player_id(), TEKUTHAL_NORMAL_OPPONENT_ID);
            assert_eq!(state.priority_player_id(), TEKUTHAL_NORMAL_OPPONENT_ID);
            assert!(state.stack.is_empty());
            assert!(state.pending_resolution.is_none());
            assert_eq!(state.static_emblems.len(), 1);
            let p29 = state
                .players
                .iter()
                .find(|player| player.id == TEKUTHAL_NORMAL_OPPONENT_ID)
                .expect("P29 remains in the post-emblem normal session");
            assert!(p29
                .hand
                .iter()
                .any(|object_id| state.objects[object_id].card_id == "divination"));
            assert!(p29
                .hand
                .iter()
                .any(|object_id| state.objects[object_id].card_id == "ornithopter"));
            let actions = kaya_batch(&latest)
                .legal_by_player
                .get(&TEKUTHAL_NORMAL_OPPONENT_ID)
                .expect("P29 receives the post-emblem current cast offers")
                .hand_actions
                .iter()
                .filter(|offer| offer.kind == HandActionKind::HandActionCastSpell as i32)
                .collect::<Vec<_>>();
            assert!(actions.iter().all(|offer| offer.card_name != "Divination"));
            assert!(actions.iter().any(|offer| offer.card_name == "Ornithopter"));
        }
        latest = narset_normal_play_land(
            &mut session,
            root.as_path(),
            &mut rows,
            &mut accepted_commands,
            &latest,
            &mut p29_battlefield,
            TEKUTHAL_NORMAL_OPPONENT_ID,
            &mut p17_islands,
            &mut p17_plains,
        );
        latest = narset_normal_float_blue(
            &mut session,
            root.as_path(),
            &mut rows,
            &mut accepted_commands,
            latest,
            &mut p29_battlefield,
            TEKUTHAL_NORMAL_OPPONENT_ID,
            3,
        );
        {
            let actions = kaya_batch(&latest)
                .legal_by_player
                .get(&TEKUTHAL_NORMAL_OPPONENT_ID)
                .expect("P29 receives current offers after floating three blue mana")
                .hand_actions
                .iter()
                .filter(|offer| offer.kind == HandActionKind::HandActionCastSpell as i32)
                .collect::<Vec<_>>();
            assert!(actions.iter().all(|offer| offer.card_name != "Divination"));
            assert!(actions.iter().any(|offer| offer.card_name == "Ornithopter"));
        }
        latest = narset_normal_cast_opponent_zero_cost(
            &mut session,
            root.as_path(),
            &mut rows,
            &mut accepted_commands,
            latest,
            &mut p29_battlefield,
            97,
        );
        latest = narset_normal_resolve_top_stack(
            &mut session,
            root.as_path(),
            &mut rows,
            &mut accepted_commands,
            latest,
            &mut p29_battlefield,
        );
        kaya_capture_battlefield(&latest, TEKUTHAL_NORMAL_OPPONENT_ID, &mut p29_battlefield);
        {
            let state = &session.engine.as_ref().unwrap().state;
            assert!(state.stack.is_empty());
            assert_eq!(state.static_emblems.len(), 1);
            assert_eq!(state.objects[&30].zone, tricerules_core::Zone::Graveyard);
            assert_eq!(state.objects[&97].zone, tricerules_core::Zone::Battlefield);
            assert_eq!(state.zone_change_generation.get(&97), Some(&2));
            assert_eq!(state.objects[&97].controller, TEKUTHAL_NORMAL_OPPONENT_ID);
            let p29 = state
                .players
                .iter()
                .find(|player| player.id == TEKUTHAL_NORMAL_OPPONENT_ID)
                .expect("P29 remains in the completed normal pilot");
            assert_eq!(
                p29.mana_pool.blue, 3,
                "zero-cost Ornithopter spends none of the blue pool"
            );
            assert!(p29
                .hand
                .iter()
                .any(|object_id| state.objects[object_id].card_id == "divination"));
        }

        fs::write(
            root.as_path().join("timeline.jsonl"),
            rows.join("\n") + "\n",
        )
        .expect("finish Narset normal-pilot timeline");
        assert_eq!(manifest["effective_dev_commands_enabled"], false);
        assert!(accepted_commands <= 8_192);
        assert!(p17_turns <= 24);
        manifest["complete"] = true.into();
        manifest["accepted_commands"] = accepted_commands.into();
        fs::write(root.as_path().join("manifest.json"), manifest.to_string())
            .expect("finish Narset normal-pilot manifest");
        let expected_state = session
            .engine
            .as_ref()
            .expect("normal Narset session has an engine after the T10 emblem and opponent cast")
            .diagnostic_snapshot()
            .expect("capture the live final diagnostic snapshot");
        let capture = load_capture(root.as_path()).expect("load completed Narset normal capture");
        assert!(
            capture.warnings.is_empty(),
            "capture warnings: {:?}",
            capture.warnings
        );
        let replayed = reconstruct(&capture, &ReplayOptions::default())
            .expect("reconstruct the complete normal Narset three-ability curve");
        assert!(
            replayed.warnings.is_empty(),
            "replay warnings: {:?}",
            replayed.warnings
        );
        assert!(
            replayed.differences.is_empty(),
            "replay differences: {:?}",
            replayed.differences
        );
        assert_eq!(
            replayed.accepted_commands,
            u64::try_from(accepted_commands).expect("accepted command count fits replay result")
        );
        assert_eq!(replayed.state, expected_state);
        println!(
            "P17 fixed-seed Narset cast/+1, T5 Rebound grant and T6 free recast, T6-T9 +1 declines, T10 -9 emblem, and P29 post-emblem free Ornithopter replayed exactly; accepted PlayerCommands={accepted_commands}, timeline records={}",
            rows.len()
        );
    }
}
