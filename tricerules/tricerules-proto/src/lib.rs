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
    fn counter_quantities_append_to_target_wire_without_changing_unused_bytes() {
        let old = [0x08, 0x11, 0x18, 0x02, 0x20, 0x02, 0x28, 0x07];
        let target = rv1::TargetRef::decode(old.as_slice()).unwrap();
        assert_eq!(target.counter_amount, 0);
        assert_eq!(target.encode_to_vec(), old);
        let mut allocated = target;
        allocated.counter_amount = 3;
        let mut expected = old.to_vec();
        expected.extend_from_slice(&[0x30, 0x03]);
        assert_eq!(allocated.encode_to_vec(), expected);
        assert_eq!(
            rv1::TargetRef::decode(expected.as_slice()).unwrap(),
            allocated
        );
        let offered = rv1::SpellTargets {
            counter_allocation: Some(rv1::CounterAllocation {
                fixed_total: 3,
                group_index: 2,
            }),
            ..Default::default()
        };
        assert_eq!(
            offered.encode_to_vec(),
            vec![0x7a, 0x04, 0x08, 0x03, 0x10, 0x02]
        );
    }

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

    fn message_descriptor<'a>(
        descriptors: &'a prost_types::FileDescriptorSet,
        name: &str,
    ) -> &'a prost_types::DescriptorProto {
        descriptors
            .file
            .iter()
            .flat_map(|file| &file.message_type)
            .find(|message| message.name.as_deref() == Some(name))
            .unwrap_or_else(|| panic!("ruled.v1.{name} is present"))
    }

    fn message_field<'a>(
        message: &'a prost_types::DescriptorProto,
        name: &str,
    ) -> &'a prost_types::FieldDescriptorProto {
        message
            .field
            .iter()
            .find(|field| field.name.as_deref() == Some(name))
            .unwrap_or_else(|| panic!("{}.{} is present", message.name.as_deref().unwrap(), name))
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

    #[test]
    fn aggregate_counter_group_wire_contract_roundtrips() {
        // These are literal canonical bytes for the reviewed additive wire shape. They are
        // intentionally decoded through the existing generated types before those types gain
        // named group members, so old prost must expose the real unknown-field loss as a RED.
        let legal = vec![
            0x08, 0x02, 0x30, 0x09, 0x5a, 0x18, 0x08, 0x03, 0x12, 0x14, 0x0a, 0x04, 0x08, 0x11,
            0x10, 0x04, 0x12, 0x0c, 0x08, 0x07, 0x12, 0x06, 0x43, 0x68, 0x61, 0x72, 0x67, 0x65,
            0x18, 0x03,
        ];
        let selection = vec![
            0x08, 0x02, 0x3a, 0x0c, 0x0a, 0x0a, 0x0a, 0x04, 0x08, 0x11, 0x10, 0x04, 0x10, 0x07,
            0x18, 0x03,
        ];
        let begin = vec![
            0x08, 0x29, 0x10, 0x04, 0x18, 0x01, 0x30, 0x01, 0x3a, 0x10, 0x08, 0x02, 0x3a, 0x0c,
            0x0a, 0x0a, 0x0a, 0x04, 0x08, 0x11, 0x10, 0x04, 0x10, 0x07, 0x18, 0x03, 0x42, 0x04,
            0x08, 0x01, 0x10, 0x01,
        ];
        let pending = vec![
            0x08, 0x05, 0x10, 0x01, 0x18, 0x02, 0x20, 0x0a, 0x28, 0x14, 0x30, 0x29, 0x38, 0x04,
            0xb2, 0x01, 0x10, 0x08, 0x02, 0x3a, 0x0c, 0x0a, 0x0a, 0x0a, 0x04, 0x08, 0x11, 0x10,
            0x04, 0x10, 0x07, 0x18, 0x03, 0xba, 0x01, 0x04, 0x08, 0x01, 0x10, 0x01,
        ];

        assert_eq!(
            rv1::LegalCostChoice::decode(legal.as_slice())
                .expect("grouped legal choice decodes")
                .encode_to_vec(),
            legal
        );
        assert_eq!(
            rv1::CostSelection::decode(selection.as_slice())
                .expect("grouped selection decodes")
                .encode_to_vec(),
            selection
        );
        assert_eq!(
            rv1::BeginAbilityActivation::decode(begin.as_slice())
                .expect("announced Begin payload decodes")
                .encode_to_vec(),
            begin
        );
        assert_eq!(
            rv1::PendingAbilityActivation::decode(pending.as_slice())
                .expect("private pending announcement decodes")
                .encode_to_vec(),
            pending
        );

        let legal = rv1::LegalCostChoice::decode(legal.as_slice()).unwrap();
        assert_eq!(legal.cost_index, 2);
        assert_eq!(legal.kind, 9);
        let group = legal
            .counter_removal_group
            .expect("grouped legal choice is typed");
        assert_eq!(group.required_count, 3);
        assert_eq!(group.candidates.len(), 1);
        let candidate = &group.candidates[0];
        let object = candidate.object.as_ref().expect("candidate has identity");
        assert_eq!((object.object_id, object.zone_change_generation), (17, 4));
        assert_eq!(candidate.options.len(), 1);
        assert_eq!(candidate.options[0].option_id, 7);
        assert_eq!(candidate.options[0].label, "Charge");
        assert_eq!(candidate.options[0].available_count, 3);

        let selection = rv1::CostSelection::decode(selection.as_slice()).unwrap();
        assert_eq!(selection.cost_index, 2);
        let group = match selection.selection.expect("grouped selection is typed") {
            rv1::cost_selection::Selection::CounterRemovalGroup(group) => group,
            other => panic!("expected grouped selection, got {other:?}"),
        };
        assert_eq!(group.contributions.len(), 1);
        let contribution = &group.contributions[0];
        let object = contribution
            .object
            .as_ref()
            .expect("contribution has identity");
        assert_eq!((object.object_id, object.zone_change_generation), (17, 4));
        assert_eq!(contribution.option_id, 7);
        assert_eq!(contribution.count, 3);

        let begin = rv1::BeginAbilityActivation::decode(begin.as_slice()).unwrap();
        assert_eq!(begin.cost_selections.len(), 1);
        assert_eq!(begin.cost_selections[0].cost_index, 2);
        assert_eq!(begin.flex_payments.len(), 1);
        assert_eq!(begin.flex_payments[0].pip_index, 1);
        assert!(begin.flex_payments[0].pay_life);

        let pending = rv1::PendingAbilityActivation::decode(pending.as_slice()).unwrap();
        assert_eq!(pending.cost_selections.len(), 1);
        assert_eq!(pending.cost_selections[0].cost_index, 2);
        assert_eq!(pending.flex_payments.len(), 1);
        assert_eq!(pending.flex_payments[0].pip_index, 1);
        assert!(pending.flex_payments[0].pay_life);

        let descriptor_bytes = include_bytes!(concat!(env!("OUT_DIR"), "/ruled-descriptors.bin"));
        let descriptors = prost_types::FileDescriptorSet::decode(descriptor_bytes.as_slice())
            .expect("the generated descriptor set decodes");
        let legal = message_field(
            message_descriptor(&descriptors, "LegalCostChoice"),
            "counter_removal_group",
        );
        assert_eq!(legal.number, Some(11));
        assert_eq!(
            legal.type_name.as_deref(),
            Some(".ruled.v1.CounterRemovalGroupChoices")
        );

        let candidate = message_descriptor(&descriptors, "CounterRemovalGroupCandidate");
        let object = message_field(candidate, "object");
        assert_eq!(object.number, Some(1));
        assert_eq!(object.type_name.as_deref(), Some(".ruled.v1.CostObjectRef"));
        let options = message_field(candidate, "options");
        assert_eq!(options.number, Some(2));
        assert_eq!(options.label, Some(3));
        assert_eq!(
            options.type_name.as_deref(),
            Some(".ruled.v1.CounterRemovalOption")
        );

        let choices = message_descriptor(&descriptors, "CounterRemovalGroupChoices");
        assert_eq!(message_field(choices, "required_count").number, Some(1));
        let candidates = message_field(choices, "candidates");
        assert_eq!(candidates.number, Some(2));
        assert_eq!(candidates.label, Some(3));
        assert_eq!(
            candidates.type_name.as_deref(),
            Some(".ruled.v1.CounterRemovalGroupCandidate")
        );

        let contribution = message_descriptor(&descriptors, "CounterRemovalContribution");
        assert_eq!(message_field(contribution, "object").number, Some(1));
        assert_eq!(message_field(contribution, "option_id").number, Some(2));
        assert_eq!(message_field(contribution, "count").number, Some(3));

        let group_selection = message_descriptor(&descriptors, "CounterRemovalGroupSelection");
        let contributions = message_field(group_selection, "contributions");
        assert_eq!(contributions.number, Some(1));
        assert_eq!(contributions.label, Some(3));
        assert_eq!(
            contributions.type_name.as_deref(),
            Some(".ruled.v1.CounterRemovalContribution")
        );

        let selection_message = message_descriptor(&descriptors, "CostSelection");
        assert_eq!(
            selection_message
                .oneof_decl
                .first()
                .and_then(|oneof| oneof.name.as_deref()),
            Some("selection")
        );
        let grouped_selection = message_field(selection_message, "counter_removal_group");
        assert_eq!(grouped_selection.number, Some(7));
        assert_eq!(grouped_selection.oneof_index, Some(0));
        assert_eq!(
            grouped_selection.type_name.as_deref(),
            Some(".ruled.v1.CounterRemovalGroupSelection")
        );

        let begin_message = message_descriptor(&descriptors, "BeginAbilityActivation");
        for (name, number, type_name) in [
            ("cost_selections", 7, ".ruled.v1.CostSelection"),
            ("flex_payments", 8, ".ruled.v1.FlexPipPayment"),
        ] {
            let field = message_field(begin_message, name);
            assert_eq!(field.number, Some(number));
            assert_eq!(field.label, Some(3));
            assert_eq!(field.type_name.as_deref(), Some(type_name));
        }

        let pending_message = message_descriptor(&descriptors, "PendingAbilityActivation");
        for (name, number, type_name) in [
            ("cost_selections", 22, ".ruled.v1.CostSelection"),
            ("flex_payments", 23, ".ruled.v1.FlexPipPayment"),
        ] {
            let field = message_field(pending_message, name);
            assert_eq!(field.number, Some(number));
            assert_eq!(field.label, Some(3));
            assert_eq!(field.type_name.as_deref(), Some(type_name));
        }

        let cost_choice_kind = descriptors
            .file
            .iter()
            .flat_map(|file| &file.enum_type)
            .find(|enumeration| enumeration.name.as_deref() == Some("CostChoiceKind"))
            .expect("ruled.v1.CostChoiceKind is present");
        assert!(cost_choice_kind.value.iter().any(|value| {
            value.name.as_deref() == Some("COST_CHOICE_KIND_REMOVE_COUNTERS_FROM_PERMANENTS")
                && value.number == Some(9)
        }));
    }

    #[test]
    fn aggregate_counter_group_absence_preserves_old_wire() {
        let old_legal = [0x08, 0x02, 0x30, 0x06];
        let old_hand_selection = [0x08, 0x02, 0x10, 0x03];
        let old_singleton_selection = [
            0x08, 0x02, 0x32, 0x08, 0x0a, 0x04, 0x08, 0x11, 0x10, 0x04, 0x10, 0x07,
        ];
        let old_begin = [0x08, 0x29, 0x10, 0x04, 0x18, 0x01, 0x30, 0x01];
        let old_pending = [
            0x08, 0x05, 0x10, 0x01, 0x18, 0x02, 0x20, 0x0a, 0x28, 0x14, 0x30, 0x29, 0x38, 0x04,
        ];

        assert_eq!(
            rv1::LegalCostChoice::decode(old_legal.as_slice())
                .unwrap()
                .encode_to_vec(),
            old_legal
        );
        assert_eq!(
            rv1::CostSelection::decode(old_hand_selection.as_slice())
                .unwrap()
                .encode_to_vec(),
            old_hand_selection
        );
        assert_eq!(
            rv1::CostSelection::decode(old_singleton_selection.as_slice())
                .unwrap()
                .encode_to_vec(),
            old_singleton_selection
        );
        assert_eq!(
            rv1::BeginAbilityActivation::decode(old_begin.as_slice())
                .unwrap()
                .encode_to_vec(),
            old_begin
        );
        assert_eq!(
            rv1::PendingAbilityActivation::decode(old_pending.as_slice())
                .unwrap()
                .encode_to_vec(),
            old_pending
        );

        assert!(rv1::LegalCostChoice::decode(old_legal.as_slice())
            .unwrap()
            .counter_removal_group
            .is_none());
        assert_eq!(
            rv1::CostSelection::decode(old_hand_selection.as_slice())
                .unwrap()
                .selection,
            Some(rv1::cost_selection::Selection::HandIndex(3))
        );
        assert!(rv1::BeginAbilityActivation::decode(old_begin.as_slice())
            .unwrap()
            .cost_selections
            .is_empty());
        assert!(rv1::BeginAbilityActivation::decode(old_begin.as_slice())
            .unwrap()
            .flex_payments
            .is_empty());
        assert!(
            rv1::PendingAbilityActivation::decode(old_pending.as_slice())
                .unwrap()
                .cost_selections
                .is_empty()
        );
        assert!(
            rv1::PendingAbilityActivation::decode(old_pending.as_slice())
                .unwrap()
                .flex_payments
                .is_empty()
        );

        let unknown_selection = [0x08, 0x02, 0x9a, 0x06, 0x00];
        let decoded = rv1::CostSelection::decode(unknown_selection.as_slice()).unwrap();
        assert_eq!(decoded.cost_index, 2);
        assert!(decoded.selection.is_none());
        assert_eq!(decoded.encode_to_vec(), [0x08, 0x02]);
        assert!(rv1::CostChoiceKind::try_from(10).is_err());

        // An explicitly present empty new message/list remains wire-present after schema
        // generation; old generated types drop these unknown fields and fail this assertion.
        let present_legal_group = [0x08, 0x02, 0x30, 0x06, 0x5a, 0x00];
        assert_eq!(
            rv1::LegalCostChoice::decode(present_legal_group.as_slice())
                .unwrap()
                .encode_to_vec(),
            present_legal_group
        );
        let present_group = rv1::LegalCostChoice::decode(present_legal_group.as_slice()).unwrap();
        assert_eq!(
            present_group
                .counter_removal_group
                .expect("present empty legal group remains present"),
            rv1::CounterRemovalGroupChoices::default()
        );
        let present_selection_group = [0x08, 0x02, 0x3a, 0x00];
        assert_eq!(
            rv1::CostSelection::decode(present_selection_group.as_slice())
                .unwrap()
                .encode_to_vec(),
            present_selection_group
        );
        assert!(matches!(
            rv1::CostSelection::decode(present_selection_group.as_slice())
                .unwrap()
                .selection,
            Some(rv1::cost_selection::Selection::CounterRemovalGroup(group))
                if group.contributions.is_empty()
        ));
        let begin_with_empty_cost = [0x08, 0x29, 0x10, 0x04, 0x18, 0x01, 0x30, 0x01, 0x3a, 0x00];
        assert_eq!(
            rv1::BeginAbilityActivation::decode(begin_with_empty_cost.as_slice())
                .unwrap()
                .encode_to_vec(),
            begin_with_empty_cost
        );
        let begin_costs = rv1::BeginAbilityActivation::decode(begin_with_empty_cost.as_slice())
            .unwrap()
            .cost_selections;
        assert_eq!(begin_costs, [rv1::CostSelection::default()]);
        let begin_with_empty_flex = [0x08, 0x29, 0x10, 0x04, 0x18, 0x01, 0x30, 0x01, 0x42, 0x00];
        assert_eq!(
            rv1::BeginAbilityActivation::decode(begin_with_empty_flex.as_slice())
                .unwrap()
                .encode_to_vec(),
            begin_with_empty_flex
        );
        let begin_flex = rv1::BeginAbilityActivation::decode(begin_with_empty_flex.as_slice())
            .unwrap()
            .flex_payments;
        assert_eq!(begin_flex, [rv1::FlexPipPayment::default()]);
        let pending_with_empty_cost = [
            0x08, 0x05, 0x10, 0x01, 0x18, 0x02, 0x20, 0x0a, 0x28, 0x14, 0x30, 0x29, 0x38, 0x04,
            0xb2, 0x01, 0x00,
        ];
        assert_eq!(
            rv1::PendingAbilityActivation::decode(pending_with_empty_cost.as_slice())
                .unwrap()
                .encode_to_vec(),
            pending_with_empty_cost
        );
        let pending_costs =
            rv1::PendingAbilityActivation::decode(pending_with_empty_cost.as_slice())
                .unwrap()
                .cost_selections;
        assert_eq!(pending_costs, [rv1::CostSelection::default()]);
        let pending_with_empty_flex = [
            0x08, 0x05, 0x10, 0x01, 0x18, 0x02, 0x20, 0x0a, 0x28, 0x14, 0x30, 0x29, 0x38, 0x04,
            0xba, 0x01, 0x00,
        ];
        assert_eq!(
            rv1::PendingAbilityActivation::decode(pending_with_empty_flex.as_slice())
                .unwrap()
                .encode_to_vec(),
            pending_with_empty_flex
        );
        let pending_flex =
            rv1::PendingAbilityActivation::decode(pending_with_empty_flex.as_slice())
                .unwrap()
                .flex_payments;
        assert_eq!(pending_flex, [rv1::FlexPipPayment::default()]);
    }

    #[test]
    fn aggregate_begin_and_pending_preserve_private_announcements() {
        let begin = [
            0x08, 0x29, 0x10, 0x04, 0x18, 0x01, 0x30, 0x01, 0x3a, 0x10, 0x08, 0x02, 0x3a, 0x0c,
            0x0a, 0x0a, 0x0a, 0x04, 0x08, 0x11, 0x10, 0x04, 0x10, 0x07, 0x18, 0x03, 0x42, 0x04,
            0x08, 0x01, 0x10, 0x01,
        ];
        let pending = [
            0x08, 0x05, 0x10, 0x01, 0x18, 0x02, 0x20, 0x0a, 0x28, 0x14, 0x30, 0x29, 0x38, 0x04,
            0xb2, 0x01, 0x10, 0x08, 0x02, 0x3a, 0x0c, 0x0a, 0x0a, 0x0a, 0x04, 0x08, 0x11, 0x10,
            0x04, 0x10, 0x07, 0x18, 0x03, 0xba, 0x01, 0x04, 0x08, 0x01, 0x10, 0x01,
        ];

        assert_eq!(
            rv1::BeginAbilityActivation::decode(begin.as_slice())
                .expect("Begin private announcement shape decodes")
                .encode_to_vec(),
            begin
        );
        assert_eq!(
            rv1::PendingAbilityActivation::decode(pending.as_slice())
                .expect("pending private announcement shape decodes")
                .encode_to_vec(),
            pending
        );
        let begin = rv1::BeginAbilityActivation::decode(begin.as_slice()).unwrap();
        assert_eq!(begin.cost_selections.len(), 1);
        assert_eq!(begin.flex_payments.len(), 1);
        assert_eq!(begin.flex_payments[0].pip_index, 1);
        assert!(begin.flex_payments[0].pay_life);
        let pending = rv1::PendingAbilityActivation::decode(pending.as_slice()).unwrap();
        assert_eq!(pending.cost_selections.len(), 1);
        assert_eq!(pending.flex_payments.len(), 1);
        assert_eq!(pending.flex_payments[0].pip_index, 1);
        assert!(pending.flex_payments[0].pay_life);
    }
    #[test]
    fn hybrid_phyrexian_color_appends_private_wire_tag_without_changing_old_mono_bytes() {
        let old = [0x08, 0x02, 0x10, 0x01];
        let mono = rv1::FlexPipPayment::decode(old.as_slice()).unwrap();
        assert_eq!(mono.pip_index, 2);
        assert!(mono.pay_life);
        assert_eq!(mono.mana_color, 0);
        assert_eq!(mono.encode_to_vec(), old);
        for (color, number) in [(rv1::ManaColor::White, 1), (rv1::ManaColor::Green, 5)] {
            let bytes = [0x08, 0x02, 0x18, number];
            let payment = rv1::FlexPipPayment::decode(bytes.as_slice()).unwrap();
            assert_eq!(payment.mana_color, color as i32);
            assert!(!payment.pay_life);
            assert_eq!(payment.encode_to_vec(), bytes);
        }
        let descriptors = prost_types::FileDescriptorSet::decode(
            include_bytes!(concat!(env!("OUT_DIR"), "/ruled-descriptors.bin")).as_slice(),
        )
        .unwrap();
        let payment = message_descriptor(&descriptors, "FlexPipPayment");
        assert_eq!(message_field(payment, "pip_index").number, Some(1));
        assert_eq!(message_field(payment, "pay_life").number, Some(2));
        let color = message_field(payment, "mana_color");
        assert_eq!(color.number, Some(3));
        assert_eq!(color.type_name.as_deref(), Some(".ruled.v1.ManaColor"));
    }
}
