//! Issue #440: printed identities for the two cards that remove every counter of every kind.
//!
//! `RemoveAllCounters` with no named kind removes all counters (CR 122.1) from one target:
//! Perfect Intimidation's second mode targets a creature and Vampire Hexmage's sacrifice ability
//! targets any permanent (pinned ruling: a suspended card in exile is not a legal target).

use tricerules_cards::primitives::{
    AbilityCost, EffectSubject, HandCardAction, HandCardChooser, HandChoiceVisibility,
    ObjectFilter, SpellEffectKind, TargetKind, TargetSchema,
};
use tricerules_cards::{Keyword, Layout};

fn every_counter_from(kind: TargetKind) -> SpellEffectKind {
    SpellEffectKind::RemoveAllCounters {
        counter: None,
        subject: EffectSubject::Chosen(Box::new(ObjectFilter {
            kind,
            ..ObjectFilter::default()
        })),
    }
}

#[test]
fn issue_440_perfect_intimidation_has_the_printed_two_modes() {
    let registry = tricerules_cards::registry::global();
    let card = registry
        .get("perfect_intimidation")
        .expect("Perfect Intimidation is registered");
    assert_eq!(card.layout, Layout::Normal);
    assert_eq!(
        registry.id_for_name("Perfect Intimidation"),
        Some("perfect_intimidation")
    );
    let face = card.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{3}{B}");
    assert_eq!(face.types, ["Sorcery"]);
    assert!(face.spell_effect.is_empty() && face.targeting.is_none());
    let modal = face.modal_spell.as_ref().expect("modal spell");
    assert_eq!(
        (modal.min_modes, modal.max_modes),
        (1, 2),
        "choose one or both"
    );
    assert_eq!(modal.modes.len(), 2);

    assert_eq!(
        modal.modes[0].effects,
        [SpellEffectKind::ChooseHandCards {
            action: HandCardAction::Exile,
            count: 2,
            target: ObjectFilter {
                kind: TargetKind::OpponentPlayer,
                ..ObjectFilter::default()
            },
            chooser: HandCardChooser::AffectedPlayer,
            card_filter: None,
            optional: false,
            visibility: HandChoiceVisibility::PrivateLook,
        }],
        "target opponent exiles two cards of their choice from their hand"
    );
    assert_eq!(
        modal.modes[1].effects,
        [every_counter_from(TargetKind::Creature)]
    );
    for mode in &modal.modes {
        let group = &mode.targeting.as_ref().expect("one target group").groups;
        assert_eq!(group.len(), 1, "{}", mode.mode_id);
        assert_eq!((group[0].min, group[0].max), (1, 1), "{}", mode.mode_id);
        TargetSchema::compile(&mode.effects, mode.targeting.as_ref())
            .unwrap_or_else(|error| panic!("{}: {error}", mode.mode_id));
    }
}

#[test]
fn issue_440_vampire_hexmage_sacrifices_to_clear_a_target_permanent() {
    let registry = tricerules_cards::registry::global();
    let card = registry
        .get("vampire_hexmage")
        .expect("Vampire Hexmage is registered");
    assert_eq!(card.layout, Layout::Normal);
    let face = card.primary_face();
    assert_eq!(face.name, "Vampire Hexmage");
    assert_eq!(face.mana_cost.to_string(), "{B}{B}");
    assert_eq!(face.types, ["Creature", "Vampire", "Shaman"]);
    assert_eq!(face.power.zip(face.toughness), Some((2, 1)));
    assert_eq!(face.keywords, [Keyword::FirstStrike]);
    assert!(face.triggered_abilities.is_empty() && face.static_abilities.is_empty());
    let [ability] = face.activated_abilities.as_slice() else {
        panic!("exactly one activated ability");
    };
    assert_eq!(ability.costs, [AbilityCost::SacrificeSelf]);
    assert_eq!(
        ability.effect,
        [every_counter_from(TargetKind::AnyPermanent)]
    );
}
