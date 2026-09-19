//! Reviewed independently against Scryfall named-card and rulings endpoints, 2026-09-19.
//! Primitive semantics and executed composition live in core scenario tests, not this mapping.
mod common;
use common::FaceExpectation;
use tricerules_cards::primitives::PlayerRecipient;
use tricerules_cards::{Amount, SpellEffectKind, TriggerCondition};

#[test]
fn pilot_draw_mappings_are_exact() {
    let divination = FaceExpectation {
        id: "divination",
        name: "Divination",
        face_id: "divination",
        mana_cost: "{2}{U}",
        types: &["Sorcery"],
        keywords: &[],
        power_toughness: None,
    }
    .check();
    assert_eq!(
        divination.spell_effect,
        [SpellEffectKind::Draw {
            who: PlayerRecipient::Controller,
            count: Amount::Fixed(2)
        }]
    );
    assert!(divination.triggered_abilities.is_empty());
    assert!(divination.activated_abilities.is_empty());
    assert!(divination.modal_spell.is_none());

    let visionary = FaceExpectation {
        id: "elvish_visionary",
        name: "Elvish Visionary",
        face_id: "elvish_visionary",
        mana_cost: "{1}{G}",
        types: &["Creature", "Elf", "Shaman"],
        keywords: &[],
        power_toughness: Some((1, 1)),
    }
    .check();
    assert!(visionary.spell_effect.is_empty());
    assert!(visionary.activated_abilities.is_empty());
    assert!(visionary.modal_spell.is_none());
    let [ability] = visionary.triggered_abilities.as_slice() else {
        panic!("exactly one ETB");
    };
    assert_eq!(ability.ability_id.as_str(), "triggered_01");
    assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert_eq!(
        ability.effect,
        [SpellEffectKind::Draw {
            who: PlayerRecipient::Controller,
            count: Amount::Fixed(1)
        }]
    );

    // The complete three-mode mapping and target schemas remain in issue_412's dedicated test.
    let pawpatch = FaceExpectation {
        id: "pawpatch_formation",
        name: "Pawpatch Formation",
        face_id: "pawpatch_formation",
        mana_cost: "{1}{G}",
        types: &["Instant"],
        keywords: &[],
        power_toughness: None,
    }
    .check();
    assert!(pawpatch.spell_effect.is_empty());
    assert!(pawpatch.triggered_abilities.is_empty());
    assert!(pawpatch.activated_abilities.is_empty());
    let modal = pawpatch.modal_spell.as_ref().unwrap();
    assert_eq!(
        (modal.min_modes, modal.max_modes, modal.modes.len()),
        (1, 1, 3)
    );
    assert_eq!(modal.modes[2].mode_id.as_str(), "mode_03");
    assert!(modal.modes[2].targeting.is_none());
    assert_eq!(
        modal.modes[2].effects,
        [
            SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(1)
            },
            SpellEffectKind::CreateTokens {
                token: "food".into(),
                count: Amount::Fixed(1),
                who: PlayerRecipient::Controller,
                tapped: false,
                sacrifice_timing: None
            },
        ]
    );
}
