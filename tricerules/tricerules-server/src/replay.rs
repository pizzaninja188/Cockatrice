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
        diagnostics::{ResumePlan, ResumeStep},
        ruled_command::Cmd,
        AutoPassPolicy, PhaseId, PlayerDeck, RuledCommand,
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
    let registry = tricerules_cards::CardRegistry::global();
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
                Ok(PlayerDeck {
                    player_id: player.id,
                    mainboard_card_name: names,
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
        == Some(tricerules_cards::CardRegistry::content_hash().as_str());
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
    use tricerules_proto::{diagnostics::Record, ipc_envelope::Msg, ruled_command::Cmd};

    fn fixture() -> (tempfile::TempDir, Value) {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("raw")).unwrap();
        let mut session = EngineSession::new(false);
        let start = IpcEnvelope {
            msg: Some(Msg::SessionStart(tricerules_proto::SessionStart {
                game_id: 7,
                seed: u64::MAX,
                player_ids: vec![17, 29],
                diagnostic_capture_enabled: true,
                ..Default::default()
            })),
        };
        let response = session.process(&start).unwrap();
        let manifest = serde_json::json!({"format_version":1,"source":"server","privacy":"server_only",
            "capture_id":"00000000-0000-4000-8000-000000000007", "engine_build":ENGINE_BUILD,
            "card_data_hash":response.card_data_hash,"effective_dev_commands_enabled":false,"complete":false});
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
                    starting_player_id: 29,
                },
            )),
        };
        let request = IpcEnvelope {
            msg: Some(Msg::PlayerCommand(tricerules_proto::PlayerCommand {
                player_id: actor,
                ruled_command: command.encode_to_vec(),
            })),
        };
        let response = session.process(&request).unwrap();
        assert!(response.ok);
        pair(root.path(), &mut rows, &request, &response);
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
}
