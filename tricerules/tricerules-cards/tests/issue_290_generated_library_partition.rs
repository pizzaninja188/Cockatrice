use std::collections::BTreeSet;

use tricerules_cards::primitives::{LibraryPartitionKind, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, CardRegistry, TriggerCondition};

const ISSUE_290_CARD_IDS: [&str; 2] = ["sage_of_days", "gurmag_nightwatch"];

#[test]
fn issue_290_registers_exactly_the_reviewed_two_card_cohort() {
    let registry = CardRegistry::global();
    for id in ISSUE_290_CARD_IDS {
        registry
            .get(id)
            .unwrap_or_else(|| panic!("missing issue #290 card {id}"));
    }

    let matching_ids = registry
        .definitions()
        .filter(|card| {
            card.faces_iter().any(|face| {
                face.triggered_abilities.iter().any(|ability| {
                    ability.trigger == TriggerCondition::WhenSelfEntersBattlefield
                        && ability.presentation == AbilityPresentation::OracleLines(vec![1])
                        && !ability.may
                        && ability.modal.is_none()
                        && ability.targeting.is_none()
                        && ability.intervening_if.is_none()
                        && matches!(
                            ability.effect.as_slice(),
                            [SpellEffectKind::LibraryPartition {
                                count: 3,
                                top_min: 0,
                                top_max: Some(1),
                                kind: LibraryPartitionKind::Look,
                            }]
                        )
                })
            })
        })
        .map(|card| card.id.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        matching_ids,
        ISSUE_290_CARD_IDS.into_iter().collect::<BTreeSet<_>>(),
        "the exact look recipe must have only its reviewed Oracle-ID cohort"
    );
}

#[test]
fn issue_290_emits_exact_etb_library_partition_with_presentation_metadata() {
    let registry = CardRegistry::global();
    for (id, name, mana, types, power, toughness, fingerprint) in [
        (
            "sage_of_days",
            "Sage of Days",
            "{2}{U}",
            ["Creature", "Human", "Wizard"],
            Some(3),
            Some(2),
            "01e53707d37c47e9e5b594a2a115a661896e3829a77872015a3d1250fbf16daa",
        ),
        (
            "gurmag_nightwatch",
            "Gurmag Nightwatch",
            "{2/B}{2/G}{2/U}",
            ["Creature", "Human", "Ranger"],
            Some(3),
            Some(3),
            "01e53707d37c47e9e5b594a2a115a661896e3829a77872015a3d1250fbf16daa",
        ),
    ] {
        let definition = registry
            .get(id)
            .unwrap_or_else(|| panic!("missing issue #290 card {id}"));
        assert_eq!(definition.name, name, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        assert_eq!(definition.face_count(), 1, "{id}");

        let face = definition.primary_face();
        assert_eq!(face.face_id.as_str(), id, "{id}");
        assert_eq!(face.name, name, "{id}");
        assert_eq!(face.mana_cost.to_string(), mana, "{id}");
        assert_eq!(
            face.types.iter().map(String::as_str).collect::<Vec<_>>(),
            types,
            "{id}"
        );
        assert_eq!((face.power, face.toughness), (power, toughness), "{id}");
        assert!(face.spell_effect.is_empty(), "{id}");
        assert!(face.targeting.is_none(), "{id}");
        assert!(face.modal_spell.is_none(), "{id}");
        assert!(face.custom_effect.is_none(), "{id}");
        assert!(face.keywords.is_empty(), "{id}");
        assert!(face.spell_keywords.is_empty(), "{id}");
        assert!(face.protections.is_empty(), "{id}");
        assert!(face.evasions.is_empty(), "{id}");
        assert!(face.activated_abilities.is_empty(), "{id}");
        assert!(face.static_abilities.is_empty(), "{id}");
        assert!(face.characteristic_defining_abilities.is_empty(), "{id}");
        assert_eq!(face.triggered_abilities.len(), 1, "{id}");

        let [ability] = face.triggered_abilities.as_slice() else {
            panic!("{id} must have one ETB ability");
        };
        assert_eq!(ability.ability_id.as_str(), "triggered_01", "{id}");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id}"
        );
        assert_eq!(
            ability.trigger,
            TriggerCondition::WhenSelfEntersBattlefield,
            "{id}"
        );
        assert!(!ability.may, "{id}");
        assert!(ability.modal.is_none(), "{id}");
        assert!(ability.targeting.is_none(), "{id}");
        assert!(ability.intervening_if.is_none(), "{id}");
        assert_eq!(
            ability.effect,
            [SpellEffectKind::LibraryPartition {
                count: 3,
                top_min: 0,
                top_max: Some(1),
                kind: LibraryPartitionKind::Look,
            }],
            "{id} effect"
        );

        let metadata = registry
            .presentation_face(id, face.face_id.as_str())
            .unwrap_or_else(|| panic!("missing presentation metadata for {id}"));
        assert_eq!(metadata.card_name, name, "{id}");
        assert_eq!(metadata.face_name, name, "{id}");
        assert_eq!(metadata.oracle_text_sha256, fingerprint, "{id}");
    }
}
