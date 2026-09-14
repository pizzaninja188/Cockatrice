use tricerules_cards::primitives::{GameCondition, PlayerRecipient, RelativePlayerSet};
use tricerules_cards::{
    AbilityPresentation, Amount, CardRegistry, SpellEffectKind, TriggerCondition,
    TriggeredAbilityDef,
};

#[test]
fn issue_278_raid_cards_have_exact_registry_shapes() {
    let registry = CardRegistry::global();

    let storm_fleet_spy = registry
        .get("storm_fleet_spy")
        .expect("Storm Fleet Spy should be generated");
    let storm_face = storm_fleet_spy.primary_face();
    assert_eq!(storm_face.mana_cost.to_string(), "{2}{U}");
    assert_eq!(storm_face.types, ["Creature", "Human", "Pirate"]);
    assert_eq!((storm_face.power, storm_face.toughness), (Some(2), Some(2)));
    assert!(storm_face.keywords.is_empty());
    assert_raid_etb_draw(storm_face.triggered_abilities.as_slice(), 1);

    let skyship_buccaneer = registry
        .get("skyship_buccaneer")
        .expect("Skyship Buccaneer should be generated");
    let skyship_face = skyship_buccaneer.primary_face();
    assert_eq!(skyship_face.mana_cost.to_string(), "{3}{U}{U}");
    assert_eq!(skyship_face.types, ["Creature", "Human", "Pirate"]);
    assert_eq!(
        (skyship_face.power, skyship_face.toughness),
        (Some(4), Some(3))
    );
    assert_eq!(skyship_face.keywords, [tricerules_cards::Keyword::Flying]);
    assert_raid_etb_draw(skyship_face.triggered_abilities.as_slice(), 2);
}

fn assert_raid_etb_draw(abilities: &[TriggeredAbilityDef], oracle_line: u16) {
    let [ability] = abilities else {
        panic!("expected one Raid ETB draw ability");
    };
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![oracle_line])
    );
    assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert_eq!(
        ability.intervening_if,
        Some(GameCondition::AttackedThisTurn {
            players: RelativePlayerSet::Controller,
        })
    );
    assert_eq!(
        ability.effect,
        [SpellEffectKind::Draw {
            who: PlayerRecipient::Controller,
            count: Amount::Fixed(1),
        }]
    );
}
