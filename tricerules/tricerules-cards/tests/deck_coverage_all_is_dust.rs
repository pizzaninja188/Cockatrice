use tricerules_cards::primitives::{EffectContext, TargetController, TargetFilter, TargetKind};
use tricerules_cards::{CardRegistry, Color, MassPlayerSet, SpellEffectKind};

#[test]
fn all_is_dust_exact_printed_kindred_eldrazi_definition_and_five_color_union() {
    let card = CardRegistry::global().get("all_is_dust").unwrap();
    let face = card.primary_face();
    assert_eq!(card.name, "All Is Dust");
    assert_eq!(face.mana_cost.to_string(), "{7}");
    assert_eq!(face.types, ["Kindred", "Sorcery", "Eldrazi"]);
    assert!(face.colors().is_empty());
    assert!(face.keywords.is_empty());
    assert!(face.targeting.is_none());
    let [SpellEffectKind::SacrificeAll { players, filter }] = face.spell_effect.as_slice() else {
        panic!("exact single mass-sacrifice instruction")
    };
    assert_eq!(*players, MassPlayerSet::All);
    let colors = filter.any_of.as_ref().unwrap();
    assert_eq!(colors.len(), 5);
    for (leaf, color) in colors.iter().zip([
        Color::White,
        Color::Blue,
        Color::Black,
        Color::Red,
        Color::Green,
    ]) {
        assert_eq!(leaf.kind, TargetKind::AnyPermanent);
        assert_eq!(leaf.is_color, Some(color));
    }
}

#[test]
fn mass_sacrifice_rejects_player_and_relative_controller_filters() {
    for filter in [
        TargetFilter {
            kind: TargetKind::AnyPlayer,
            ..Default::default()
        },
        TargetFilter {
            kind: TargetKind::AnyTarget,
            ..Default::default()
        },
        TargetFilter {
            kind: TargetKind::Creature,
            controller: TargetController::You,
            ..Default::default()
        },
    ] {
        assert!(SpellEffectKind::SacrificeAll {
            players: MassPlayerSet::All,
            filter
        }
        .validate(EffectContext::Spell)
        .is_err());
    }
    assert!(SpellEffectKind::SacrificeAll {
        players: MassPlayerSet::TargetedPlayer {
            group_index: 0,
            kind: TargetKind::AnyPlayer
        },
        filter: TargetFilter {
            kind: TargetKind::Creature,
            ..Default::default()
        },
    }
    .validate(EffectContext::Spell)
    .is_ok());
    assert!(SpellEffectKind::SacrificeAll {
        players: MassPlayerSet::TargetedPlayer {
            group_index: 0,
            kind: TargetKind::Creature
        },
        filter: TargetFilter {
            kind: TargetKind::Creature,
            ..Default::default()
        },
    }
    .validate(EffectContext::Spell)
    .is_err());
}
