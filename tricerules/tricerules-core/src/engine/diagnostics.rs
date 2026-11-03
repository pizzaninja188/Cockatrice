//! Maintainer-only, read-only state evidence. Never include this in recipient events.

use super::GameEngine;
use serde_json::Value;

impl GameEngine {
    pub fn diagnostic_snapshot(&self) -> Result<Value, serde_json::Error> {
        let mut objects: Vec<_> = self.state.objects.values().collect();
        objects.sort_by_key(|object| object.id);
        let identities = objects.into_iter().map(|object| {
            let derived = if object.zone == crate::state::Zone::Battlefield {
                crate::diagnostic_json::to_value(&self.characteristics(object.id))?
            } else { Value::Null };
            Ok(serde_json::json!({
                "engine_object_id": object.id,
                "card_definition_id": object.card_id,
                "known_name": self.registry.get(&object.card_id).map(|card| &card.name),
                "zone_change_generation": self.state.zone_change_generation.get(&object.id).copied().unwrap_or(0).to_string(),
                "face_index": object.face_up_index.to_string(),
                "derived_characteristics": derived,
            }))
        }).collect::<Result<Vec<Value>, serde_json::Error>>()?;
        let mut state = crate::diagnostic_json::to_value(&self.state)?;
        // Captures keep their established schema while internal state groups related facts.
        // Raw state serialization still includes the complete store for atomicity checks.
        let state_fields = state
            .as_object_mut()
            .expect("GameState serializes as an object");
        state_fields.remove("last_known_information");
        self.state
            .last_known_information
            .extend_legacy_diagnostic(state_fields)?;
        state_fields.remove("mana_restriction_groups");
        let groups: Vec<_> = self
            .state
            .mana_restriction_groups
            .iter()
            .map(|(_, group)| group)
            .collect();
        state_fields.insert(
            "mana_restrictions".into(),
            crate::diagnostic_json::to_value(
                &groups
                    .iter()
                    .map(|group| &group.restriction)
                    .collect::<Vec<_>>(),
            )?,
        );
        state_fields.insert(
            "mana_restriction_presentations".into(),
            crate::diagnostic_json::to_value(
                &groups
                    .iter()
                    .map(|group| &group.presentation)
                    .collect::<Vec<_>>(),
            )?,
        );
        state_fields.insert(
            "mana_restriction_origins".into(),
            crate::diagnostic_json::to_value(
                &groups.iter().map(|group| &group.origin).collect::<Vec<_>>(),
            )?,
        );
        Ok(serde_json::json!({
            "format_version": 1,
            "privacy": "server_only",
            "dev_commands_enabled": self.dev_commands_enabled,
            "state": state,
            "object_identities": identities,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_is_complete_lossless_and_deterministic() {
        let mut a = GameEngine::new(
            tricerules_cards::registry::global(),
            u64::MAX,
            &[17, 29],
            20,
            None,
            false,
        )
        .unwrap();
        let b = GameEngine::new(
            tricerules_cards::registry::global(),
            u64::MAX,
            &[17, 29],
            20,
            None,
            false,
        )
        .unwrap();
        let first = a.diagnostic_snapshot().unwrap();
        assert_eq!(first["state"]["seed"], u64::MAX.to_string());
        assert_eq!(first["state"]["players"][0]["id"], 17);
        assert!(!first["state"]["objects"].as_object().unwrap().is_empty());
        assert!(first["state"]["opening"].is_object());
        assert!(first["object_identities"][0]["known_name"]
            .as_str()
            .is_some_and(|name| !name.is_empty()));
        assert_eq!(first["state"]["pending_resolution"], Value::Null);
        assert!(first["state"]
            .get("last_known_tapped_by_generation")
            .is_some());
        assert_eq!(first, b.diagnostic_snapshot().unwrap());
        a.state
            .last_known_information
            .record_tapped((42, u64::MAX), true);
        a.state.skip_next_untap.insert((42, u64::MAX));
        let second = a.diagnostic_snapshot().unwrap();
        assert_ne!(first, second);
        assert!(second.to_string().contains("18446744073709551615"));
        assert_eq!(
            a.state.command_index, 0,
            "inspection cannot advance gameplay"
        );
    }

    #[test]
    fn protocol_state_values_use_descriptor_enum_labels() {
        let phase = tricerules_proto::PhaseChanged {
            active_player_id: 17,
            phase_id: tricerules_proto::PhaseId::Main1 as i32,
        };
        let json = crate::diagnostic_json::to_value(&phase).unwrap();
        assert_eq!(json["phase_id"], "PHASE_ID_MAIN1");
    }

    #[test]
    fn lki_legacy_diagnostics_preserve_sparse_facts_and_partial_updates() {
        use std::collections::{BTreeMap, HashMap};
        use tricerules_card_model::{Color, CounterKind, Keyword};

        let mut engine = GameEngine::new(
            tricerules_cards::registry::global(),
            u64::MAX,
            &[17, 29],
            20,
            None,
            false,
        )
        .unwrap();
        let initial = engine.diagnostic_snapshot().unwrap();
        let old = (42, 1);
        let later = (42, u64::MAX);
        let partial = (90, 3);
        let object_id = *engine.state.players[0].library.front().unwrap();
        let copy = super::super::copying::token_copy_snapshot_from(
            &engine.state,
            engine.registry,
            object_id,
        )
        .unwrap();
        let attachment = crate::state::AttachedObjectLki {
            object_id: 99,
            zone_change_generation: u64::MAX,
            owner: 0,
        };
        let mut expected_fields = serde_json::Map::new();
        macro_rules! record {
            ($legacy:ident, $record:ident, $entries:expr) => {{
                let expected = HashMap::from($entries);
                for (key, value) in expected.clone() {
                    engine.state.last_known_information.$record(key, value);
                }
                expected_fields.insert(
                    stringify!($legacy).into(),
                    crate::diagnostic_json::to_value(&expected).unwrap(),
                );
            }};
        }
        record!(last_known_tapped, record_latest_tapped, [(42, true)]);
        record!(
            last_known_tapped_by_generation,
            record_tapped,
            [(old, true), (later, false)]
        );
        record!(
            last_known_keywords_by_generation,
            record_keywords,
            [
                (old, vec![]),
                (later, vec![Keyword::Flying]),
                (partial, vec![Keyword::Lifelink])
            ]
        );
        record!(
            last_known_colors_by_generation,
            record_colors,
            [(old, vec![]), (later, vec![Color::Blue])]
        );
        record!(
            last_known_types_by_generation,
            record_types,
            [(old, vec![]), (later, vec!["Artifact".into()])]
        );
        record!(
            last_known_names_by_generation,
            record_names,
            [(old, vec![])]
        );
        record!(
            last_known_controller_by_generation,
            record_controller,
            [(old, 0), (later, 17)]
        );
        record!(
            last_known_counters_by_generation,
            record_counters,
            [
                (old, BTreeMap::new()),
                (later, BTreeMap::from([(CounterKind::PlusOnePlusOne, 2)]))
            ]
        );
        record!(
            last_known_pt_by_generation,
            record_power_toughness,
            [(old, (None, None)), (later, (Some(-2), None))]
        );
        record!(
            last_known_mana_value_by_generation,
            record_mana_value,
            [(old, 0)]
        );
        record!(
            last_known_copy_by_generation,
            record_copy_snapshot,
            [(old, copy)]
        );
        record!(
            last_known_attached_object_by_generation,
            record_attached_object,
            [(later, attachment)]
        );

        assert_eq!(
            engine.state.last_known_information.tapped(&later),
            Some(&false)
        );
        assert_eq!(
            engine.state.last_known_information.latest_tapped(&42),
            Some(&true)
        );
        assert_eq!(
            engine.state.last_known_information.power_toughness(&old),
            Some(&(None, None))
        );
        assert!(engine
            .state
            .last_known_information
            .keywords(&old)
            .unwrap()
            .is_empty());
        assert!(engine
            .state
            .last_known_information
            .controller(&partial)
            .is_none());
        assert!(engine
            .state
            .last_known_information
            .counters(&partial)
            .is_none());

        let snapshot = engine.diagnostic_snapshot().unwrap();
        let state = &snapshot["state"];
        assert_eq!(state["_type"], "GameState");
        assert!(state.get("last_known_information").is_none());
        for (field, expected) in &expected_fields {
            assert_eq!(
                initial["state"][field],
                serde_json::json!({}),
                "{field} starts as an empty map"
            );
            assert_eq!(
                &state[field], expected,
                "{field} retains the legacy map shape"
            );
        }
        assert_eq!(
            state["last_known_tapped_by_generation"]["entries"][1]["key"],
            serde_json::json!([42, "18446744073709551615"])
        );
        assert_eq!(
            state["last_known_pt_by_generation"]["entries"][1]["value"],
            serde_json::json!(["-2", null])
        );
        assert_eq!(
            state["last_known_copy_by_generation"]["entries"][0]["value"]["_type"],
            "TokenCopySnapshot"
        );
        assert_eq!(
            state["last_known_attached_object_by_generation"]["entries"][0]["value"]["_type"],
            "AttachedObjectLki"
        );
        let mut initial_state = initial["state"].as_object().unwrap().clone();
        let mut populated_state = state.as_object().unwrap().clone();
        for field in expected_fields.keys() {
            initial_state.remove(field);
            populated_state.remove(field);
        }
        assert_eq!(
            initial_state, populated_state,
            "unrelated diagnostic fields remain unchanged"
        );

        // Phasing and departing players record derived qualities without inventing departure facts.
        let characteristics = engine.characteristics(object_id).unwrap();
        super::super::resolution::record_last_known_characteristics(
            &mut engine.state,
            old.0,
            old.1,
            characteristics,
        );
        let updated = engine.diagnostic_snapshot().unwrap();
        for field in [
            "last_known_tapped",
            "last_known_tapped_by_generation",
            "last_known_counters_by_generation",
            "last_known_copy_by_generation",
            "last_known_attached_object_by_generation",
        ] {
            assert_eq!(
                updated["state"][field], state[field],
                "derived updates retain {field}"
            );
        }
        assert_eq!(
            engine.state.last_known_information.power_toughness(&later),
            Some(&(Some(-2), None))
        );
        assert!(engine
            .state
            .last_known_information
            .controller(&partial)
            .is_none());
        let raw_state = crate::diagnostic_json::to_value(&engine.state).unwrap();
        assert_eq!(
            raw_state["last_known_information"]["objects"]["entries"]
                .as_array()
                .unwrap()
                .len(),
            3,
            "raw serialization retains the complete sparse store for rejection snapshots"
        );
        let existing_mana_value = *engine
            .state
            .last_known_information
            .mana_value(&old)
            .unwrap();
        engine
            .state
            .last_known_information
            .record_mana_value_if_absent(old, 99);
        assert_eq!(
            engine.state.last_known_information.mana_value(&old),
            Some(&existing_mana_value)
        );
        engine
            .state
            .last_known_information
            .record_mana_value_if_absent(partial, 0);
        engine
            .state
            .last_known_information
            .record_mana_value_if_absent(partial, 99);
        assert_eq!(
            engine.state.last_known_information.mana_value(&partial),
            Some(&0)
        );
        assert!(engine
            .state
            .last_known_information
            .tapped(&partial)
            .is_none());
        assert_eq!(
            engine.state.last_known_information.keywords(&partial),
            Some(&vec![Keyword::Lifelink])
        );
    }

    #[test]
    fn parked_resolution_serialization_and_execution_transfer_contract() {
        use crate::state::{
            OperationResults, ParkedStackResolution, ProgramFrame, ProgramRunState, ProgramValue,
        };
        use tricerules_card_model::primitives::CountResultKind;
        use tricerules_card_model::{CardFaceId, ProgramOwnerId};

        let engine = GameEngine::new(
            tricerules_cards::registry::global(),
            707,
            &[17, 29],
            20,
            None,
            false,
        )
        .unwrap();
        let object_id = *engine.state.players[0].library.front().unwrap();
        let mut item = engine.observer_return_item(object_id, 17);
        item.source_zone_change = u64::MAX;
        let previous = OperationResults::from_count(CountResultKind::RevealedPrefix, 7);
        let run = ProgramRunState {
            owner: ProgramOwnerId::face("parked_contract", CardFaceId::new("front").unwrap()),
            pending_owners: vec![ProgramOwnerId::face(
                "parked_contract",
                CardFaceId::new("back").unwrap(),
            )],
            frames: vec![ProgramFrame {
                path: vec![],
                next_instruction: 2,
                yield_to: vec![],
            }],
            slots: vec![None, Some(ProgramValue::Count(5))],
            in_flight: Some(vec![]),
            in_flight_results: OperationResults::from_count(CountResultKind::TappedPermanents, 3),
        };
        let dedicated = ParkedStackResolution::dedicated(item.clone());
        let flat = ParkedStackResolution::for_flat_effects(item.clone(), Some(2), previous.clone());
        // Shared continuation paths can retain a Program and a fallback flat cursor together.
        let program = ParkedStackResolution::for_program(
            item.clone(),
            Some(9),
            previous.clone(),
            run.clone(),
        );
        for (source, index, facts, program_state) in [
            (&dedicated, None, OperationResults::default(), None),
            (&flat, Some(2), previous.clone(), None),
            (&program, Some(9), previous.clone(), Some(&run)),
        ] {
            let expected = serde_json::json!({
                "_type": "ParkedStackResolution",
                "item": crate::diagnostic_json::to_value(&item).unwrap(),
                "resume_effect_index": index,
                "previous_result": crate::diagnostic_json::to_value(&facts).unwrap(),
                "program": crate::diagnostic_json::to_value(&program_state).unwrap(),
            });
            assert_eq!(crate::diagnostic_json::to_value(source).unwrap(), expected);
            assert_eq!(
                serde_json::to_value(source).unwrap(),
                serde_json::json!({
                    "item": serde_json::to_value(&item).unwrap(),
                    "resume_effect_index": index,
                    "previous_result": serde_json::to_value(&facts).unwrap(),
                    "program": serde_json::to_value(program_state).unwrap(),
                })
            );

            let receiver_item = engine.observer_return_item(object_id + 1000, 29);
            let mut receiver = program.clone();
            receiver.item = receiver_item.clone();
            receiver.restore_execution_from(source);
            let mut expected_receiver = expected;
            expected_receiver["item"] = crate::diagnostic_json::to_value(&receiver_item).unwrap();
            assert_eq!(
                crate::diagnostic_json::to_value(&receiver).unwrap(),
                expected_receiver,
                "restoring execution retains the receiver's replacement-specific stack item"
            );
        }

        let mut retained = program;
        let completed = OperationResults::from_count(CountResultKind::JoinForcesManaPaid, 11);
        retained.set_flat_tail(Some(4), completed.clone());
        assert_eq!(
            crate::diagnostic_json::to_value(&retained.program()).unwrap(),
            crate::diagnostic_json::to_value(&Some(&run)).unwrap(),
            "paired flat-tail updates preserve the owning Program and its partial operation facts"
        );
        assert_eq!(
            crate::diagnostic_json::to_value(&retained.item).unwrap(),
            crate::diagnostic_json::to_value(&item).unwrap()
        );
        assert_eq!(
            crate::diagnostic_json::to_value(retained.previous_result()).unwrap(),
            crate::diagnostic_json::to_value(&completed).unwrap()
        );
    }
}
