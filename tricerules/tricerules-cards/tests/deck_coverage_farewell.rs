use tricerules_cards::primitives::{EffectContext, SpellEffectKind, TargetFilter, TargetKind};
use tricerules_cards::Color;

#[test]
fn farewell_exact_white_sorcery_identity_and_one_to_four_printed_order_modes() {
    let card = tricerules_cards::registry::global()
        .get("farewell")
        .unwrap();
    let face = card.primary_face();
    assert_eq!(card.name, "Farewell");
    assert_eq!(face.mana_cost.to_string(), "{4}{W}{W}");
    assert_eq!(face.types, ["Sorcery"]);
    assert_eq!(face.colors(), [Color::White]);
    let modal = face.modal_spell.as_ref().unwrap();
    assert_eq!((modal.min_modes, modal.max_modes), (1, 4));
    assert_eq!(
        modal
            .modes
            .iter()
            .map(|m| m.mode_id.as_str())
            .collect::<Vec<_>>(),
        ["artifacts", "creatures", "enchantments", "graveyards"]
    );
    assert!(matches!(
        &modal.modes[3].effects[0],
        SpellEffectKind::ExileGraveyards { .. }
    ));
    for mode in &modal.modes {
        assert!(mode
            .effects
            .iter()
            .all(|effect| effect.target_roles().is_empty()));
    }
}

#[test]
fn mass_exile_rejects_player_and_relative_owner_controller_filters() {
    use tricerules_cards::primitives::TargetController;
    for kind in [
        TargetKind::AnyPlayer,
        TargetKind::AnyTarget,
        TargetKind::OpponentPlayer,
    ] {
        let effect = SpellEffectKind::ExileAll {
            kind: TargetFilter {
                kind,
                ..Default::default()
            },
        };
        assert!(effect.validate(EffectContext::Spell).is_err());
    }
    let effect = SpellEffectKind::ExileAll {
        kind: TargetFilter {
            kind: TargetKind::AnyPermanent,
            controller: TargetController::You,
            ..Default::default()
        },
    };
    assert!(effect.validate(EffectContext::Spell).is_err());
    assert!(SpellEffectKind::ExileAll {
        kind: TargetFilter {
            kind: TargetKind::Creature,
            ..Default::default()
        }
    }
    .validate(EffectContext::Spell)
    .is_ok());
}
