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
            BeginSpellCast, CastMethod, CastSource, ChoiceKind, CommitSpellCast, CostObjectRef,
            DevAddMana, DevCommand, DevMoveCard, DevPutCardInZone, DevZone, DiscardToHandSize,
            HandActionKind, LandSource, PassPriority, PaymentMana, PaymentSelection, PlayLand,
            PresentationPathKind, ResolutionChoiceDecision, SelectedSpellMode,
            SubmitResolutionChoice, TargetRef, TargetRefKind,
        },
        ruled_command::Cmd,
        ChooseStartingPlayer, CommanderSetup, MulliganDecision, PaymentQuery, PlayerCommand,
        PlayerDeck, PreviewPayment, RuledCommand, SessionStart, ValidateDeck,
    };

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
}
