//! Registry conformance for the queue-v70 batch A cohorts.
//!
//! Three stable typed shapes: a one-shot pump with a printed `flashback_cost`, a tap-and-skip-
//! untap spell with one rider, and a land with one enters-the-battlefield trigger and one `{T}`
//! mana ability. Fingerprints are added after the presentation metadata refresh.

use tricerules_cards::primitives::{
    Amount, DiscardQuantity, EffectSubject, EntersTappedAffected, Keyword, SpellEffectKind,
    StaticAbilityDef, TriggerCondition,
};
use tricerules_cards::{
    registry, AbilityCost, AbilityPresentation, AbilitySourceZone, CardFace, Layout,
};

fn face(id: &str, name: &str, mana: &str, types: &[&str]) -> &'static CardFace {
    let definition = registry::global()
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"));
    assert_eq!(definition.layout, Layout::Normal, "{id}");
    assert_eq!(registry::global().id_for_name(name), Some(id), "{id}");
    let f = definition.primary_face();
    assert_eq!(f.name, name, "{id} name");
    assert_eq!(f.mana_cost.to_string(), mana, "{id} mana");
    assert_eq!(
        f.types.iter().map(String::as_str).collect::<Vec<_>>(),
        types,
        "{id} types"
    );
    f
}

fn assert_target_group(f: &CardFace, min: u32, max: u32, effect_indices: &[u32], id: &str) {
    let targeting = f
        .targeting
        .as_ref()
        .unwrap_or_else(|| panic!("{id}: targeting"));
    let group = targeting
        .groups
        .first()
        .unwrap_or_else(|| panic!("{id}: target group"));
    assert_eq!((group.min, group.max), (min, max), "{id}: target bound");
    assert_eq!(group.effect_indices, effect_indices, "{id}: group effects");
}

fn assert_activated<'a>(
    f: &'a CardFace,
    index: usize,
    line: usize,
    id: &str,
) -> &'a tricerules_cards::ActivatedAbilityDef {
    let ability = f
        .activated_abilities
        .get(index)
        .unwrap_or_else(|| panic!("{id}: missing activated ability {index}"));
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![line as u16]),
        "{id}: presentation"
    );
    ability
}

#[test]
fn q70_batch_a_cohorts_map_definitions() {
    // 01: pump plus an optional keyword grant, plus the printed flashback alternative cost.
    for (id, name, mana, types, power, toughness, keyword, flashback) in [
        (
            "reckless_charge",
            "Reckless Charge",
            "{R}",
            &["Sorcery"][..],
            3,
            0,
            Some(Keyword::Haste),
            "{2}{R}",
        ),
        (
            "sylvan_might",
            "Sylvan Might",
            "{1}{G}",
            &["Instant"][..],
            2,
            2,
            Some(Keyword::Trample),
            "{2}{G}{G}",
        ),
        (
            "thrill_of_the_hunt",
            "Thrill of the Hunt",
            "{G}",
            &["Instant"][..],
            1,
            2,
            None,
            "{W}",
        ),
        (
            "wild_hunger",
            "Wild Hunger",
            "{2}{G}",
            &["Instant"][..],
            3,
            1,
            Some(Keyword::Trample),
            "{3}{R}",
        ),
    ] {
        let f = face(id, name, mana, types);
        assert_eq!(
            f.flashback_cost
                .as_ref()
                .map(ToString::to_string)
                .as_deref(),
            Some(flashback),
            "{id}: printed flashback cost"
        );
        match keyword {
            Some(keyword) => {
                assert!(
                    matches!(
                        f.spell_effect.as_slice(),
                        [
                            SpellEffectKind::PumpTarget {
                                power: actual_power,
                                toughness: actual_toughness,
                                subject: EffectSubject::Chosen(_),
                                ..
                            },
                            SpellEffectKind::GrantKeywords { keywords, .. }
                        ] if *actual_power == power
                            && *actual_toughness == toughness
                            && keywords.as_slice() == [keyword]
                    ),
                    "{id}: {:?}",
                    f.spell_effect
                );
                assert_target_group(f, 1, 1, &[0, 1], id);
            }
            None => {
                assert!(
                    matches!(
                        f.spell_effect.as_slice(),
                        [SpellEffectKind::PumpTarget {
                            power: actual_power,
                            toughness: actual_toughness,
                            subject: EffectSubject::Chosen(_),
                            ..
                        }] if *actual_power == power && *actual_toughness == toughness
                    ),
                    "{id}: {:?}",
                    f.spell_effect
                );
                assert_target_group(f, 1, 1, &[0], id);
            }
        }
    }

    // 02: tap and skip the next untap step, with a rider on three of the four cards.
    for (id, name, mana, types, min, max, rider) in [
        (
            "take_into_custody",
            "Take into Custody",
            "{U}",
            &["Instant"][..],
            1u32,
            1u32,
            "",
        ),
        (
            "press_for_answers",
            "Press for Answers",
            "{1}{U}",
            &["Sorcery"][..],
            1,
            1,
            "clue",
        ),
        (
            "frostveil_ambush",
            "Frostveil Ambush",
            "{3}{U}{U}",
            &["Instant"][..],
            0,
            2,
            "cycling",
        ),
        (
            "snow_day",
            "Snow Day",
            "{4}{U}{U}",
            &["Instant"][..],
            0,
            2,
            "draw_discard",
        ),
    ] {
        let f = face(id, name, mana, types);
        assert!(
            matches!(
                f.spell_effect.as_slice(),
                [
                    SpellEffectKind::Tap {
                        subject: EffectSubject::Chosen(_)
                    },
                    SpellEffectKind::SkipNextUntap { .. },
                    ..
                ]
            ),
            "{id}: {:?}",
            f.spell_effect
        );
        assert_target_group(f, min, max, &[0, 1], id);
        match rider {
            "" => assert_eq!(f.spell_effect.len(), 2, "{id}"),
            "clue" => assert!(
                matches!(
                    f.spell_effect.as_slice(),
                    [_, _, SpellEffectKind::CreateTokens { token, count, .. }]
                        if token == "clue" && *count == Amount::Fixed(1)
                ),
                "{id}: {:?}",
                f.spell_effect
            ),
            "draw_discard" => assert!(
                matches!(
                    f.spell_effect.as_slice(),
                    [
                        _,
                        _,
                        SpellEffectKind::Draw {
                            count: Amount::Fixed(2),
                            ..
                        },
                        SpellEffectKind::Discard {
                            quantity: DiscardQuantity::Exact(1),
                            ..
                        }
                    ]
                ),
                "{id}: {:?}",
                f.spell_effect
            ),
            "cycling" => {
                assert_eq!(f.spell_effect.len(), 2, "{id}");
                let cycling = assert_activated(f, 0, 2, id);
                assert_eq!(cycling.source_zone, AbilitySourceZone::Hand, "{id}");
                assert!(
                    matches!(
                        cycling.costs.as_slice(),
                        [AbilityCost::Mana(cost), AbilityCost::DiscardSelf]
                            if cost.to_string() == "{1}"
                    ),
                    "{id}: Cycling {{1}}"
                );
                assert!(matches!(
                    cycling.effect.as_slice(),
                    [SpellEffectKind::Draw {
                        count: Amount::Fixed(1),
                        ..
                    }]
                ));
            }
            other => panic!("{id}: unknown rider {other}"),
        }
    }

    // 03: land entry trigger plus one mana ability.
    for (id, name, color, tapped, trigger) in [
        ("radiant_fountain", "Radiant Fountain", 'c', false, "life"),
        ("kabira_crossroads", "Kabira Crossroads", 'w', true, "life"),
        ("new_benalia", "New Benalia", 'w', true, "scry"),
        ("zhalfirin_void", "Zhalfirin Void", 'c', false, "scry"),
    ] {
        let f = face(id, name, "", &["Land"]);
        let [triggered] = f.triggered_abilities.as_slice() else {
            panic!("{id}: exactly one triggered ability")
        };
        assert_eq!(
            triggered.trigger,
            TriggerCondition::WhenSelfEntersBattlefield,
            "{id}"
        );
        assert_eq!(
            triggered.presentation,
            AbilityPresentation::OracleLines(vec![if tapped { 2 } else { 1 }]),
            "{id}: trigger line"
        );
        match trigger {
            "life" => assert!(
                matches!(
                    triggered.effect.as_slice(),
                    [SpellEffectKind::GainLife {
                        amount: Amount::Fixed(2),
                        ..
                    }]
                ),
                "{id}: {:?}",
                triggered.effect
            ),
            "scry" => assert!(
                matches!(
                    triggered.effect.as_slice(),
                    [SpellEffectKind::Scry {
                        count: Amount::Fixed(1)
                    }]
                ),
                "{id}: {:?}",
                triggered.effect
            ),
            other => panic!("{id}: unknown trigger {other}"),
        }
        assert!(triggered.targeting.is_none(), "{id}: untargeted trigger");

        let mana = assert_activated(f, 0, if tapped { 3 } else { 2 }, id);
        assert!(matches!(mana.costs.as_slice(), [AbilityCost::Tap]));
        assert!(
            matches!(
                mana.effect.as_slice(),
                [SpellEffectKind::ProduceMana { options, .. }]
                    if options.len() == 1
                        && match color {
                            'c' => options[0].c == 1,
                            'w' => options[0].w == 1,
                            _ => false,
                        }
            ),
            "{id}: printed mana"
        );
        if tapped {
            assert_eq!(f.static_abilities.len(), 1, "{id}: one replacement");
            assert_eq!(
                f.static_abilities[0].definition,
                StaticAbilityDef::EntersTapped {
                    affected: EntersTappedAffected::Self_,
                    condition: None,
                    unless_cost: None,
                },
                "{id}: enters tapped"
            );
        } else {
            assert!(f.static_abilities.is_empty(), "{id}: no entry replacement");
        }
    }
}

#[test]
fn q70_batch_a_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, fingerprint) in [
        (
            "reckless_charge",
            "Reckless Charge",
            "2ac9ffe1c02872eae7ee25ded70668e8637088022cb5a94d2d8aacd25a549fef",
        ),
        (
            "sylvan_might",
            "Sylvan Might",
            "e882183a43be346f8fa617476d2a73ff78dc9132c1b318cb7ce0470ac55ea952",
        ),
        (
            "thrill_of_the_hunt",
            "Thrill of the Hunt",
            "4e6554315943196c292f45c02aabc381b0c0647b6d684e7dd3b9955eb26bb886",
        ),
        (
            "wild_hunger",
            "Wild Hunger",
            "53863a36a5690292c1136aae22721f02ca447907935f177a7304678b967d6a8d",
        ),
        (
            "take_into_custody",
            "Take into Custody",
            "1b3141eaffaade76aad8232e3631f1dca99d65caf1ff9045b0169d2496aab6a4",
        ),
        (
            "press_for_answers",
            "Press for Answers",
            "9092209dbe0639efa073d925898eefe68b135dfa46ef70c12b7bad86f20b0613",
        ),
        (
            "frostveil_ambush",
            "Frostveil Ambush",
            "68b231746ed8ef9a93d7d7188ba36609e79b3a19b668b474150b2674637117a3",
        ),
        (
            "snow_day",
            "Snow Day",
            "8404493f1e3657b60d4888883b66ec4040631104c931fab54dacf923bc2a32fd",
        ),
        (
            "radiant_fountain",
            "Radiant Fountain",
            "01244d4d17f9805fdade29fc2a03f243c97a0fe74896916e071be81043e29027",
        ),
        (
            "kabira_crossroads",
            "Kabira Crossroads",
            "5e907769185838a4db7ad5c36dab0d821d3cb092ab2c916524338c520bff98b7",
        ),
        (
            "new_benalia",
            "New Benalia",
            "e1350e599db556b14be70245983ca5a628653bff0881b6dfc1b5f704802af613",
        ),
        (
            "zhalfirin_void",
            "Zhalfirin Void",
            "b1d1381c44888757dc6de2183bdb66172fdbf1e3972d0d9cdfc3faeb151bc964",
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
        assert_eq!(fields[2], id, "face id drift for {id}");
        assert_eq!(fields[3], name, "display name drift for {id}");
        assert_eq!(fields[4], fingerprint, "pinned Oracle fingerprint for {id}");
    }
}
