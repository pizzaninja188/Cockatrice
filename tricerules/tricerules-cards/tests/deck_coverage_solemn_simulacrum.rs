use tricerules_cards::primitives::{
    Amount, CardTypeFilter, PlayerRecipient, SearchDestination, SpellEffectKind, TriggerCondition,
    ZoneCardFilter,
};
use tricerules_cards::{AbilityPresentation, CardRegistry, Layout};

#[test]
fn solemn_simulacrum_registers_both_optional_triggered_abilities() {
    let registry = CardRegistry::global();
    let card = registry
        .get("solemn_simulacrum")
        .expect("Solemn Simulacrum registry definition");

    assert_eq!(card.name, "Solemn Simulacrum");
    assert_eq!(
        registry.id_for_name("Solemn Simulacrum"),
        Some("solemn_simulacrum")
    );
    assert_eq!(card.layout, Layout::Normal);
    assert_eq!(card.face_count(), 1);

    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "solemn_simulacrum");
    assert_eq!(face.mana_cost.to_string(), "{4}");
    assert_eq!(face.types, ["Artifact", "Creature", "Golem"]);
    assert!(face.colors().is_empty());
    assert_eq!((face.power, face.toughness), (Some(2), Some(2)));
    assert!(face.spell_effect.is_empty());
    assert!(face.activated_abilities.is_empty());
    assert_eq!(face.triggered_abilities.len(), 2);

    let entry = &face.triggered_abilities[0];
    assert_eq!(entry.ability_id.as_str(), "triggered_01");
    assert_eq!(
        entry.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert_eq!(entry.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert!(matches!(
        entry.effect.as_slice(),
        [SpellEffectKind::ChooseResolutionBranch {
            optional: true,
            branches,
            ..
        }] if branches.len() == 1
            && branches[0].branch_id.as_str() == "search_for_a_basic_land"
            && branches[0].presentation == AbilityPresentation::OracleLines(vec![1])
            && matches!(
                branches[0].effects.as_slice(),
                [SpellEffectKind::SearchLibrary {
                    filter: Some(ZoneCardFilter {
                        card_type: Some(CardTypeFilter::BasicLand),
                        ..
                    }),
                    destination: SearchDestination::Battlefield { tapped: true },
                    shuffle: true,
                    reveal: false,
                    ..
                }]
            )
    ));

    let death = &face.triggered_abilities[1];
    assert_eq!(death.ability_id.as_str(), "triggered_02");
    assert_eq!(
        death.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(death.trigger, TriggerCondition::WhenSelfDies);
    assert!(matches!(
        death.effect.as_slice(),
        [SpellEffectKind::ChooseResolutionBranch {
            optional: true,
            branches,
            ..
        }] if branches.len() == 1
            && branches[0].branch_id.as_str() == "draw_a_card"
            && branches[0].presentation == AbilityPresentation::OracleLines(vec![2])
            && matches!(
                branches[0].effects.as_slice(),
                [SpellEffectKind::Draw {
                    who: PlayerRecipient::Controller,
                    count: Amount::Fixed(1),
                }]
            )
    ));
}
