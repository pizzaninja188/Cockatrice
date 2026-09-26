use tricerules_cards::primitives::{AbilityCost, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, AbilitySourceZone, CardRegistry, Layout, ManaAmount};

#[test]
fn lotus_petal_registers_its_free_artifact_and_five_color_mana_ability() {
    let registry = CardRegistry::global();
    let card = registry
        .get("lotus_petal")
        .expect("Lotus Petal registry definition");

    assert_eq!(card.name, "Lotus Petal");
    assert_eq!(registry.id_for_name("Lotus Petal"), Some("lotus_petal"));
    assert_eq!(card.layout, Layout::Normal);
    assert_eq!(card.face_count(), 1);

    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "lotus_petal");
    assert_eq!(face.mana_cost.to_string(), "{0}");
    assert_eq!(face.types, ["Artifact"]);
    assert!(face.colors().is_empty());
    assert!(face.spell_effect.is_empty());
    assert!(face.triggered_abilities.is_empty());
    assert_eq!(face.activated_abilities.len(), 1);

    let ability = &face.activated_abilities[0];
    assert_eq!(ability.ability_id.as_str(), "activated_01");
    assert_eq!(ability.source_zone, AbilitySourceZone::Battlefield);
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(
        ability.costs,
        [AbilityCost::Tap, AbilityCost::SacrificeSelf]
    );
    let expected = [
        ManaAmount {
            w: 1,
            ..ManaAmount::default()
        },
        ManaAmount {
            u: 1,
            ..ManaAmount::default()
        },
        ManaAmount {
            b: 1,
            ..ManaAmount::default()
        },
        ManaAmount {
            r: 1,
            ..ManaAmount::default()
        },
        ManaAmount {
            g: 1,
            ..ManaAmount::default()
        },
    ];
    assert!(matches!(
        ability.effect.as_slice(),
        [SpellEffectKind::ProduceMana { options, .. }] if options.as_slice() == expected
    ));
}
