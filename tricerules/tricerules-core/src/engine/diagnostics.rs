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
        Ok(serde_json::json!({
            "format_version": 1,
            "privacy": "server_only",
            "dev_commands_enabled": self.dev_commands_enabled,
            "state": crate::diagnostic_json::to_value(&self.state)?,
            "object_identities": identities,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn snapshot_is_complete_lossless_and_deterministic() {
        let mut a = GameEngine::new(u64::MAX, &[17, 29], 20, None, false).unwrap();
        let b = GameEngine::new(u64::MAX, &[17, 29], 20, None, false).unwrap();
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
            .last_known_tapped_by_generation
            .insert((42, u64::MAX), true);
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
}
