//! Registry conformance for the queue-v64 batch A cohorts.
//!
//! Three stable typed shapes: an opponent-wide shrink plus draw or cycling, a conditional self
//! first strike during your turn with at most one activated pump, and an uncounterable creature
//! with one printed keyword. Fingerprints are asserted once the presentation metadata is refreshed
//! from the pinned Oracle text.

use tricerules_cards::primitives::{
    AbilityCost, AbilitySourceZone, Amount, Color, CreatureScopeController, CreatureScopeFilter,
    GameCondition, Keyword, ProtectionQuality, RelativePlayerSet, SpellEffectKind,
    StaticAbilityDef,
};
use tricerules_cards::{registry, CardFace, Layout};

fn face(id: &str, name: &str, mana: &str, types: &[&str]) -> &'static CardFace {
    let definition = registry::global()
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"));
    assert_eq!(definition.layout, Layout::Normal, "{id}");
    assert_eq!(registry::global().id_for_name(name), Some(id), "{id}");
    let f = definition.primary_face();
    assert_eq!(
        f.face_id.as_str(),
        id.replace([',', '-'], "_").replace("__", "_").as_str(),
        "{id} face id"
    );
    assert_eq!(f.name, name, "{id} name");
    assert_eq!(f.mana_cost.to_string(), mana, "{id} mana");
    assert_eq!(
        f.types.iter().map(String::as_str).collect::<Vec<_>>(),
        types,
        "{id} types"
    );
    f
}

#[test]
fn q64_opponent_shrink_map_definitions() {
    for (id, name, mana, shrink, rider) in [
        (
            "blinding_spray",
            "Blinding Spray",
            "{4}{U}",
            (-4, 0),
            "draw",
        ),
        (
            "hampering_snare",
            "Hampering Snare",
            "{1}{U}",
            (-2, 0),
            "cycle",
        ),
        (
            "suffocating_fumes",
            "Suffocating Fumes",
            "{2}{B}",
            (-1, -1),
            "cycle",
        ),
    ] {
        let f = face(id, name, mana, &["Instant"]);
        assert!(f.targeting.is_none(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
        assert!(f.triggered_abilities.is_empty(), "{id}");
        let [shrink_effect, rest @ ..] = f.spell_effect.as_slice() else {
            panic!("{id}: opponent shrink");
        };
        assert!(
            matches!(
                shrink_effect,
                SpellEffectKind::PumpAll {
                    filter: CreatureScopeFilter {
                        controller: Some(CreatureScopeController::Opponents),
                        ..
                    },
                    power,
                    toughness,
                } if (*power, *toughness) == shrink
            ),
            "{id}: exact printed opponent shrink: {shrink_effect:?}"
        );
        match rider {
            "draw" => {
                let [SpellEffectKind::Draw {
                    count: Amount::Fixed(1),
                    ..
                }] = rest
                else {
                    panic!("{id}: one draw: {rest:?}");
                };
                assert_eq!(f.spell_effect.len(), 2, "{id}");
                assert!(f.activated_abilities.is_empty(), "{id}");
            }
            "cycle" => {
                assert!(rest.is_empty(), "{id}: only the shrink: {rest:?}");
                let [ability] = f.activated_abilities.as_slice() else {
                    panic!("{id}: one cycling ability");
                };
                assert_eq!(ability.source_zone, AbilitySourceZone::Hand, "{id}");
                assert!(
                    matches!(
                        ability.costs.as_slice(),
                        [AbilityCost::Mana(cost), AbilityCost::DiscardSelf]
                            if cost.to_string() == "{2}"
                    ),
                    "{id}: printed cycling cost"
                );
                assert!(
                    matches!(
                        ability.effect.as_slice(),
                        [SpellEffectKind::Draw {
                            count: Amount::Fixed(1),
                            ..
                        }]
                    ),
                    "{id}: cycling draws one"
                );
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn q64_during_turn_first_strike_map_definitions() {
    for (id, name, mana, types, power, toughness) in [
        (
            "fresh-faced_recruit",
            "Fresh-Faced Recruit",
            "{1}{R/W}",
            ["Creature", "Human", "Soldier"].as_slice(),
            2,
            1,
        ),
        (
            "pouncing_lynx",
            "Pouncing Lynx",
            "{1}{W}",
            ["Creature", "Cat"].as_slice(),
            2,
            1,
        ),
        (
            "spartan_veteran",
            "Spartan Veteran",
            "{R}",
            ["Creature", "Human", "Soldier"].as_slice(),
            1,
            1,
        ),
    ] {
        let f = face(id, name, mana, types);
        assert_eq!(
            (f.power, f.toughness),
            (Some(power), Some(toughness)),
            "{id}"
        );
        let [static_ability] = f.static_abilities.as_slice() else {
            panic!("{id}: one conditional self modifier");
        };
        assert_eq!(
            static_ability.presentation,
            tricerules_cards::AbilityPresentation::OracleLines(vec![1]),
            "{id}"
        );
        assert!(
            matches!(
                &static_ability.definition,
                StaticAbilityDef::ConditionalSelfModifier {
                    condition: GameCondition::ActivePlayer {
                        players: RelativePlayerSet::Controller
                    },
                    keywords,
                    ..
                } if keywords.as_slice() == [Keyword::FirstStrike]
            ),
            "{id}: during-your-turn first strike"
        );
        if id == "spartan_veteran" {
            let [ability] = f.activated_abilities.as_slice() else {
                panic!("{id}: one activated pump");
            };
            assert!(
                matches!(
                    ability.costs.as_slice(),
                    [AbilityCost::Mana(cost)] if cost.to_string() == "{2}"
                ),
                "{id}: printed activation cost"
            );
            assert!(
                matches!(
                    ability.effect.as_slice(),
                    [SpellEffectKind::PumpTarget {
                        power: 1,
                        toughness: 0,
                        subject: tricerules_cards::primitives::EffectSubject::Source,
                        ..
                    }]
                ),
                "{id}: printed self pump"
            );
        } else {
            assert!(f.activated_abilities.is_empty(), "{id}");
        }
        assert!(f.triggered_abilities.is_empty(), "{id}");
    }
}

#[test]
fn q64_uncounterable_creatures_map_definitions() {
    for (id, name, mana, types, power, toughness) in [
        (
            "blurred_mongoose",
            "Blurred Mongoose",
            "{1}{G}",
            ["Creature", "Mongoose"].as_slice(),
            2,
            1,
        ),
        (
            "scragnoth",
            "Scragnoth",
            "{4}{G}",
            ["Creature", "Beast"].as_slice(),
            3,
            4,
        ),
        (
            "terra_stomper",
            "Terra Stomper",
            "{3}{G}{G}{G}",
            ["Creature", "Beast"].as_slice(),
            8,
            8,
        ),
    ] {
        let f = face(id, name, mana, types);
        assert_eq!(
            (f.power, f.toughness),
            (Some(power), Some(toughness)),
            "{id}"
        );
        let [static_ability] = f.static_abilities.as_slice() else {
            panic!("{id}: one uncounterable static");
        };
        assert!(
            matches!(
                static_ability.definition,
                StaticAbilityDef::SpellCannotBeCountered
            ),
            "{id}: this spell can't be countered"
        );
        match id {
            "blurred_mongoose" => {
                assert_eq!(f.keywords.as_slice(), [Keyword::Shroud], "{id}");
                assert!(f.protections.is_empty(), "{id}");
            }
            "scragnoth" => {
                assert!(f.keywords.is_empty(), "{id}");
                assert_eq!(
                    f.protections.as_slice(),
                    [ProtectionQuality::Color(Color::Blue)],
                    "{id}"
                );
            }
            "terra_stomper" => {
                assert_eq!(f.keywords.as_slice(), [Keyword::Trample], "{id}");
                assert!(f.protections.is_empty(), "{id}");
            }
            _ => unreachable!(),
        }
        assert!(f.activated_abilities.is_empty(), "{id}");
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.targeting.is_none(), "{id}");
    }
}

#[test]
fn q64_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, face_id, fingerprint) in [
        (
            "blinding_spray",
            "Blinding Spray",
            "blinding_spray",
            "9919470916b1587251510676756667a9806ec32f510eb68a5785f8448430075d",
        ),
        (
            "blurred_mongoose",
            "Blurred Mongoose",
            "blurred_mongoose",
            "7b43df6d20ec94bfe2bfc4c06659e48aca9f9f66a35ef4a07a8044eba5975053",
        ),
        (
            "fresh-faced_recruit",
            "Fresh-Faced Recruit",
            "fresh_faced_recruit",
            "132249a7786afe8f507f35b1cbf6c669607aa20d38d4760fbce911b4c22e7617",
        ),
        (
            "hampering_snare",
            "Hampering Snare",
            "hampering_snare",
            "873de6a43da173b5311a5fb627906d001fe83b656989d41770d3fec0c7200850",
        ),
        (
            "pouncing_lynx",
            "Pouncing Lynx",
            "pouncing_lynx",
            "132249a7786afe8f507f35b1cbf6c669607aa20d38d4760fbce911b4c22e7617",
        ),
        (
            "scragnoth",
            "Scragnoth",
            "scragnoth",
            "10d54acfa46805707e04b7c035e37ca09e96843a03fd90034cc5d00187d859cb",
        ),
        (
            "spartan_veteran",
            "Spartan Veteran",
            "spartan_veteran",
            "4c7f1c8fb94164d4be12789bc929cf7362b3f72cd01c3302588b129801fa2e3e",
        ),
        (
            "suffocating_fumes",
            "Suffocating Fumes",
            "suffocating_fumes",
            "087afe81faa4e5938886f6900ea00acf51878560e83ba09074d1098d7aaa8909",
        ),
        (
            "terra_stomper",
            "Terra Stomper",
            "terra_stomper",
            "42c25fa7543dec3a7a4ffd785564933e84877d72534c3947638ff5deb2983cc0",
        ),
    ] {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(fields[2], face_id);
        assert_eq!(fields[4], fingerprint, "fingerprint drift for {id}");
    }
}
