#![allow(clippy::derive_partial_eq_without_eq)]
#![allow(clippy::large_enum_variant)]

/// Prost emits flat items for `package ruled.v1`; nest them to match the proto package path.
pub mod ruled {
    pub mod v1 {
        include!(concat!(env!("OUT_DIR"), "/ruled.v1.rs"));
    }
    pub mod diagnostics {
        include!(concat!(env!("OUT_DIR"), "/ruled.diagnostics.rs"));
    }
}

pub use ruled::v1::*;
pub mod diagnostic_enums;

pub use ruled::diagnostics;

/// Maximum number of external card names carried by one private card-look log.
pub const PRIVATE_CARD_LOOK_MAX_CARD_NAMES: usize = 64;
/// Maximum UTF-8 byte length of one external card name in a private card-look log.
pub const PRIVATE_CARD_LOOK_MAX_NAME_BYTES: usize = 512;

/// Validate the owner-private routing contract for a typed card-look log.
///
/// Ordinary logs without `private_card_look` preserve their existing behavior. A typed look is
/// accepted only when the nested nonnegative viewer matches the whole-message owner route, no
/// hidden-recipient route conflicts with it, and every external display name is bounded text.
pub fn validate_private_card_look(message: &ruled::v1::LogMessage) -> Result<(), &'static str> {
    let Some(look) = &message.private_card_look else {
        return Ok(());
    };
    let Some(viewer) = look.viewer_player_id else {
        return Err("PrivateCardLook requires an explicit viewer player id");
    };
    if viewer < 0 {
        return Err("PrivateCardLook viewer player id must be nonnegative");
    }
    let Some(route_viewer) = message.visible_to_player_id else {
        return Err("PrivateCardLook requires owner-only visible_to_player_id routing");
    };
    if route_viewer != viewer || message.hidden_from_player_id.is_some() {
        return Err("PrivateCardLook routing metadata does not match its viewer");
    }
    if look.external_card_names.is_empty()
        || look.external_card_names.len() > PRIVATE_CARD_LOOK_MAX_CARD_NAMES
    {
        return Err("PrivateCardLook must contain between 1 and 64 card names");
    }
    if look
        .external_card_names
        .iter()
        .any(|name| name.trim().is_empty() || name.len() > PRIVATE_CARD_LOOK_MAX_NAME_BYTES)
    {
        return Err("PrivateCardLook card names must be nonempty and at most 512 UTF-8 bytes");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::ruled::v1 as rv1;
    use super::{
        validate_private_card_look, PRIVATE_CARD_LOOK_MAX_CARD_NAMES,
        PRIVATE_CARD_LOOK_MAX_NAME_BYTES,
    };
    use prost::Message;

    #[test]
    fn empty_target_pair_constraint_preserves_presence_through_wire_roundtrip() {
        let ordinary = rv1::LegalTargetGroup::default();
        let constrained = rv1::LegalTargetGroup {
            pair_constraint: Some(rv1::TargetPairConstraint {
                prior_group_index: 0,
                compatible_pairs: vec![],
            }),
            ..Default::default()
        };
        for group in [ordinary, constrained] {
            let decoded = rv1::LegalTargetGroup::decode(group.encode_to_vec().as_slice()).unwrap();
            assert_eq!(decoded, group);
        }
    }

    #[test]
    fn presentation_reference_round_trips_every_identity_and_mapping_component() {
        let reference = rv1::PresentationRef {
            card_id: "grow_from_the_ashes".into(),
            face_id: "grow_from_the_ashes".into(),
            path: vec![
                rv1::PresentationPathComponent {
                    kind: rv1::PresentationPathKind::Spell as i32,
                    id: "spell".into(),
                },
                rv1::PresentationPathComponent {
                    kind: rv1::PresentationPathKind::CastCostGroup as i32,
                    id: "kicker".into(),
                },
                rv1::PresentationPathComponent {
                    kind: rv1::PresentationPathKind::CastCostOption as i32,
                    id: "kicked".into(),
                },
            ],
            oracle_line_indices: vec![1, 3],
            fallback_text: "Kicker {2}".into(),
            external_card_name: "Grow from the Ashes".into(),
            external_face_name: "Grow from the Ashes".into(),
            oracle_text_sha256: "0123456789abcdef".repeat(4),
        };

        let decoded = rv1::PresentationRef::decode(reference.encode_to_vec().as_slice())
            .expect("presentation reference decodes");
        assert_eq!(decoded, reference);
    }

    #[test]
    fn loyalty_x_offer_preserves_zero_range_presence_and_descriptor_fields() {
        let no_choice = rv1::AbilityInfo::default();
        let zero_range = rv1::AbilityInfo {
            loyalty_x_choice: Some(rv1::ability_info::LoyaltyXChoice { min_x: 0, max_x: 0 }),
            ..Default::default()
        };
        assert!(
            rv1::AbilityInfo::decode(no_choice.encode_to_vec().as_slice())
                .unwrap()
                .loyalty_x_choice
                .is_none()
        );
        assert_eq!(
            rv1::AbilityInfo::decode(zero_range.encode_to_vec().as_slice()).unwrap(),
            zero_range,
            "a present 0..=0 offer stays distinct from no LoyaltyX offer"
        );

        let descriptor_bytes = include_bytes!(concat!(env!("OUT_DIR"), "/ruled-descriptors.bin"));
        let descriptors = prost_types::FileDescriptorSet::decode(descriptor_bytes.as_slice())
            .expect("the generated descriptor set decodes");
        let ability = descriptors
            .file
            .iter()
            .flat_map(|file| &file.message_type)
            .find(|message| message.name.as_deref() == Some("AbilityInfo"))
            .expect("ruled.v1.AbilityInfo is present");
        assert!(ability.field.iter().any(|field| {
            field.name.as_deref() == Some("loyalty_x_choice") && field.number == Some(12)
        }));
        let choice = ability
            .nested_type
            .iter()
            .find(|message| message.name.as_deref() == Some("LoyaltyXChoice"))
            .expect("AbilityInfo.LoyaltyXChoice is present");
        for (name, number) in [("min_x", 1), ("max_x", 2)] {
            assert!(choice
                .field
                .iter()
                .any(|field| field.name.as_deref() == Some(name) && field.number == Some(number)));
        }
    }

    #[test]
    fn log_message_declares_the_owner_private_card_look_contract() {
        let descriptor_bytes = include_bytes!(concat!(env!("OUT_DIR"), "/ruled-descriptors.bin"));
        let descriptors = prost_types::FileDescriptorSet::decode(descriptor_bytes.as_slice())
            .expect("the generated descriptor set decodes");
        let log = descriptors
            .file
            .iter()
            .flat_map(|file| &file.message_type)
            .find(|message| message.name.as_deref() == Some("LogMessage"))
            .expect("ruled.v1.LogMessage is present");
        assert!(log.field.iter().any(|field| {
            field.name.as_deref() == Some("private_card_look") && field.number == Some(5)
        }));

        let look = descriptors
            .file
            .iter()
            .flat_map(|file| &file.message_type)
            .find(|message| message.name.as_deref() == Some("PrivateCardLook"))
            .expect("the typed private look message is present");
        for name in ["viewer_player_id", "external_card_names"] {
            assert!(
                look.field
                    .iter()
                    .any(|field| field.name.as_deref() == Some(name)),
                "PrivateCardLook includes {name}"
            );
        }
        let viewer = look
            .field
            .iter()
            .find(|field| field.name.as_deref() == Some("viewer_player_id"))
            .expect("the private look has a viewer field");
        assert_eq!(viewer.proto3_optional, Some(true));
    }

    fn private_look_log(viewer: Option<i32>, names: Vec<String>) -> rv1::LogMessage {
        rv1::LogMessage {
            text: "Inspect the looked-at cards".into(),
            visible_to_player_id: viewer,
            hidden_from_player_id: None,
            ability_presentation: None,
            private_card_look: Some(rv1::PrivateCardLook {
                viewer_player_id: viewer,
                external_card_names: names,
            }),
        }
    }

    #[test]
    fn private_card_look_round_trips_and_enforces_owner_routing_and_bounds() {
        let valid = private_look_log(Some(0), vec!["Opt".into(), "Island".into()]);
        assert!(validate_private_card_look(&valid).is_ok());
        let decoded = rv1::LogMessage::decode(valid.encode_to_vec().as_slice())
            .expect("the private look log decodes");
        assert_eq!(decoded, valid);

        let at_limits = private_look_log(
            Some(1),
            vec!["x".repeat(PRIVATE_CARD_LOOK_MAX_NAME_BYTES); PRIVATE_CARD_LOOK_MAX_CARD_NAMES],
        );
        assert!(validate_private_card_look(&at_limits).is_ok());
        assert!(validate_private_card_look(&private_look_log(
            Some(1),
            vec!["é".repeat(PRIVATE_CARD_LOOK_MAX_NAME_BYTES / 2)],
        ))
        .is_ok());
        assert!(validate_private_card_look(&private_look_log(
            Some(1),
            vec!["é".repeat(PRIVATE_CARD_LOOK_MAX_NAME_BYTES / 2 + 1)],
        ))
        .is_err());

        assert!(
            validate_private_card_look(&private_look_log(Some(-1), vec!["Opt".into()])).is_err()
        );
        let mut missing_viewer = private_look_log(Some(1), vec!["Opt".into()]);
        missing_viewer
            .private_card_look
            .as_mut()
            .unwrap()
            .viewer_player_id = None;
        assert!(validate_private_card_look(&missing_viewer).is_err());
        let mut missing_route = private_look_log(Some(1), vec!["Opt".into()]);
        missing_route.visible_to_player_id = None;
        assert!(validate_private_card_look(&missing_route).is_err());
        let mut mismatched_route = private_look_log(Some(1), vec!["Opt".into()]);
        mismatched_route.visible_to_player_id = Some(2);
        assert!(validate_private_card_look(&mismatched_route).is_err());
        let mut conflicting_route = private_look_log(Some(1), vec!["Opt".into()]);
        conflicting_route.hidden_from_player_id = Some(2);
        assert!(validate_private_card_look(&conflicting_route).is_err());
        assert!(validate_private_card_look(&private_look_log(Some(1), vec![])).is_err());
        assert!(validate_private_card_look(&private_look_log(Some(1), vec!["  ".into()])).is_err());
        assert!(validate_private_card_look(&private_look_log(
            Some(1),
            vec!["x".repeat(PRIVATE_CARD_LOOK_MAX_NAME_BYTES + 1)],
        ))
        .is_err());
        assert!(validate_private_card_look(&private_look_log(
            Some(1),
            vec!["x".into(); PRIVATE_CARD_LOOK_MAX_CARD_NAMES + 1],
        ))
        .is_err());
        assert!(validate_private_card_look(&rv1::LogMessage::default()).is_ok());
    }
}
