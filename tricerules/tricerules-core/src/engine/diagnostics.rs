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
}
