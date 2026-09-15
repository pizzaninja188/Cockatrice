use std::collections::BTreeSet;

use tricerules_cards::primitives::{
    HandCardAction, HandCardChooser, HandChoiceVisibility, SpellEffectKind, TargetFilter,
    TargetKind,
};
use tricerules_cards::{AbilityPresentation, CardRegistry, TriggerCondition};

const ISSUE_288_CARD_IDS: [&str; 2] = ["skullcap_snail", "unscrupulous_agent"];

#[test]
fn issue_288_registers_exactly_the_reviewed_two_card_cohort() {
    let registry = CardRegistry::global();
    for id in ISSUE_288_CARD_IDS {
        registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
    }

    let matching_ids = registry
        .definitions()
        .filter(|card| {
            card.faces_iter().any(|face| {
                face.triggered_abilities.iter().any(|ability| {
                    matches!(
                        ability.effect.as_slice(),
                        [SpellEffectKind::ChooseHandCards {
                            action: HandCardAction::Exile,
                            count: 1,
                            target: TargetFilter {
                                kind: TargetKind::OpponentPlayer,
                                ..
                            },
                            chooser: HandCardChooser::AffectedPlayer,
                            card_filter: None,
                            optional: false,
                            visibility: HandChoiceVisibility::PrivateLook,
                        }]
                    )
                })
            })
        })
        .map(|card| card.id.as_str())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        matching_ids,
        ISSUE_288_CARD_IDS.into_iter().collect::<BTreeSet<_>>(),
        "the exact exile recipe must have only its reviewed Oracle-ID cohort"
    );
}

#[test]
fn issue_288_emits_exact_private_affected_player_exile_trigger() {
    let registry = CardRegistry::global();
    for id in ISSUE_288_CARD_IDS {
        let card = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        let face = card.primary_face();
        assert_eq!(face.triggered_abilities.len(), 1, "{id}");
        assert!(face.activated_abilities.is_empty(), "{id}");
        assert!(face.static_abilities.is_empty(), "{id}");
        assert!(face.characteristic_defining_abilities.is_empty(), "{id}");
        assert!(face.spell_effect.is_empty(), "{id}");
        assert!(face.modal_spell.is_none(), "{id}");
        assert!(face.custom_effect.is_none(), "{id}");

        let ability = &face.triggered_abilities[0];
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
        assert!(ability.intervening_if.is_none(), "{id}");
        assert_eq!(ability.effect.len(), 1, "{id}");
        let SpellEffectKind::ChooseHandCards {
            action,
            count,
            target,
            chooser,
            card_filter,
            optional,
            visibility,
        } = &ability.effect[0]
        else {
            panic!("{id} must emit ChooseHandCards");
        };
        assert_eq!(*action, HandCardAction::Exile, "{id}");
        assert_eq!(*count, 1, "{id}");
        assert_eq!(target.kind, TargetKind::OpponentPlayer, "{id}");
        assert_eq!(*chooser, HandCardChooser::AffectedPlayer, "{id}");
        assert!(card_filter.is_none(), "{id}");
        assert!(!*optional, "{id}");
        assert_eq!(*visibility, HandChoiceVisibility::PrivateLook, "{id}");

        let targeting = ability.targeting.as_ref().expect("one opponent target");
        assert_eq!(targeting.groups.len(), 1, "{id}");
        let group = &targeting.groups[0];
        assert_eq!((group.min, group.max), (1, 1), "{id}");
        assert_eq!(group.prompt, "Choose target opponent", "{id}");
        assert_eq!(group.effect_indices, [0], "{id}");
        assert!(group.distinct_from.is_empty(), "{id}");
    }
}
