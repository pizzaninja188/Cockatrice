use tricerules_cards::primitives::{
    AbilityCost, CounterKind, GameCondition, ManaAmount, SpellEffectKind, StaticAbilityDef,
};
use tricerules_cards::AbilityPresentation;

#[test]
fn gemstone_caverns_exact_identity_opening_profile_and_conditional_mana() {
    let registry = tricerules_cards::registry::global();
    assert_eq!(
        registry.id_for_name("Gemstone Caverns"),
        Some("gemstone_caverns")
    );
    let card = registry.get("gemstone_caverns").unwrap();
    assert!(!card.commander_setup_only);
    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "gemstone_caverns");
    assert_eq!(face.supertypes, ["Legendary"]);
    assert_eq!(face.types, ["Land"]);
    assert!(face.mana_cost.is_empty());
    assert_eq!(face.static_abilities.len(), 1);
    assert_eq!(
        face.static_abilities[0].definition,
        StaticAbilityDef::GemstoneCavernsOpening
    );
    assert_eq!(
        face.static_abilities[0].presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(face.activated_abilities.len(), 1);
    let mana = &face.activated_abilities[0];
    assert_eq!(mana.costs, [AbilityCost::Tap]);
    assert_eq!(mana.presentation, AbilityPresentation::OracleLines(vec![2]));
    let [SpellEffectKind::ProduceMana {
        options,
        conditional: Some(conditional),
        restriction: None,
        commander_color_identity: false,
    }] = mana.effect.as_slice()
    else {
        panic!("exact unrestricted conditional mana ability required")
    };
    assert_eq!(
        options,
        &[ManaAmount {
            c: 1,
            ..Default::default()
        }]
    );
    assert_eq!(
        conditional.condition,
        GameCondition::SourceCounterCount {
            counter: CounterKind::Luck,
            min: Some(1),
            max: None
        }
    );
    assert_eq!(
        conditional.options,
        vec![
            ManaAmount {
                w: 1,
                ..Default::default()
            },
            ManaAmount {
                u: 1,
                ..Default::default()
            },
            ManaAmount {
                b: 1,
                ..Default::default()
            },
            ManaAmount {
                r: 1,
                ..Default::default()
            },
            ManaAmount {
                g: 1,
                ..Default::default()
            }
        ]
    );
}
