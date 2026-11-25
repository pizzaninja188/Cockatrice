//! Registry conformance for the queue-v69 batch A cohorts.
//!
//! Three stable typed shapes: a black draw-two-and-lose-two spell with an optional scry rider, a
//! small Wizard with exactly two one-colored-mana `{C}, {T}` activated abilities, and an artifact
//! whose `{T}` mana ability offers all five colors with one additional clause. Fingerprints are
//! added after the presentation metadata refresh.

use tricerules_cards::primitives::{
    Amount, EffectSubject, GraveyardDestination, LifeAmount, SpellEffectKind, TriggerCondition,
};
use tricerules_cards::{registry, AbilityCost, AbilityPresentation, CardFace, Keyword, Layout};

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

fn activated<'a>(
    f: &'a CardFace,
    index: usize,
    id: &str,
) -> &'a tricerules_cards::ActivatedAbilityDef {
    f.activated_abilities
        .get(index)
        .unwrap_or_else(|| panic!("{id}: missing activated ability {index}"))
}

fn assert_oracle_line(ability: &tricerules_cards::ActivatedAbilityDef, line: usize, id: &str) {
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![line as u16]),
        "{id}"
    );
}

fn assert_costs(ability: &tricerules_cards::ActivatedAbilityDef, expected: &[&str], id: &str) {
    let actual: Vec<String> = ability
        .costs
        .iter()
        .map(|cost| match cost {
            AbilityCost::Mana(mana) => format!("Mana({mana})"),
            AbilityCost::Tap => "Tap".to_string(),
            AbilityCost::SacrificeSelf => "SacrificeSelf".to_string(),
            other => format!("{other:?}"),
        })
        .collect();
    assert_eq!(actual, expected, "{id}");
}

fn assert_any_color_mana(f: &CardFace, index: usize, id: &str) {
    let ability = activated(f, index, id);
    let [SpellEffectKind::ProduceMana { options, .. }] = ability.effect.as_slice() else {
        panic!("{id}: {:?}", ability.effect);
    };
    assert_eq!(options.len(), 5, "{id}");
    let mut seen = [0u32; 5];
    for option in options {
        assert_eq!(
            (option.w, option.u, option.b, option.r, option.g, option.c),
            (
                u32::from(option.w != 0),
                u32::from(option.u != 0),
                u32::from(option.b != 0),
                u32::from(option.r != 0),
                u32::from(option.g != 0),
                0
            ),
            "{id}: one unit of a single color per option"
        );
        for (slot, value) in [option.w, option.u, option.b, option.r, option.g]
            .into_iter()
            .enumerate()
        {
            seen[slot] += u32::from(value != 0);
        }
    }
    assert_eq!(seen, [1, 1, 1, 1, 1], "{id}: all five colors are offered");
}

#[test]
fn q69_batch_a_cohorts_map_definitions() {
    // 01: Night's Whisper is an effect list; the two Scry 2 spells need an ordered program so the
    // scry choice precedes the draw.
    let whisper = face("nights_whisper", "Night's Whisper", "{1}{B}", &["Sorcery"]);
    assert!(
        matches!(
            whisper.spell_effect.as_slice(),
            [
                SpellEffectKind::Draw {
                    count: Amount::Fixed(2),
                    ..
                },
                SpellEffectKind::LoseLife {
                    amount: LifeAmount::Fixed(2),
                    ..
                }
            ]
        ),
        "{:?}",
        whisper.spell_effect
    );
    for (id, name, mana, types) in [
        ("demons_due", "Demon's Due", "{3}{B}", &["Instant"][..]),
        (
            "read_the_bones",
            "Read the Bones",
            "{2}{B}",
            &["Sorcery"][..],
        ),
    ] {
        let f = face(id, name, mana, types);
        assert!(f.spell_effect.is_empty(), "{id}: {:?}", f.spell_effect);
        let program = f.spell_program.as_ref().unwrap_or_else(|| panic!("{id}"));
        assert_eq!(program.body.steps.len(), 3, "{id}: scry, draw, lose life");
    }

    // 02: each card has exactly two activated abilities, each costing one colored mana plus tap.
    for (id, name, mana, types, p, t, first_cost, second_cost) in [
        (
            "armorer_guildmage",
            "Armorer Guildmage",
            "{R}",
            &["Creature", "Human", "Wizard"][..],
            1u32,
            1u32,
            "{B}",
            "{G}",
        ),
        (
            "bant_battlemage",
            "Bant Battlemage",
            "{2}{W}",
            &["Creature", "Human", "Wizard"][..],
            2,
            2,
            "{G}",
            "{U}",
        ),
        (
            "naya_battlemage",
            "Naya Battlemage",
            "{2}{G}",
            &["Creature", "Human", "Shaman"][..],
            2,
            2,
            "{R}",
            "{W}",
        ),
        (
            "senate_guildmage",
            "Senate Guildmage",
            "{W}{U}",
            &["Creature", "Human", "Wizard"][..],
            2,
            2,
            "{W}",
            "{U}",
        ),
        (
            "shaper_guildmage",
            "Shaper Guildmage",
            "{U}",
            &["Creature", "Human", "Wizard"][..],
            1,
            1,
            "{W}",
            "{B}",
        ),
    ] {
        let f = face(id, name, mana, types);
        assert_eq!((f.power, f.toughness), (Some(p), Some(t)), "{id}");
        assert!(f.keywords.is_empty(), "{id}");
        assert_eq!(f.activated_abilities.len(), 2, "{id}");
        let first = activated(f, 0, id);
        let second = activated(f, 1, id);
        assert_costs(first, &[&format!("Mana({first_cost})"), "Tap"], id);
        assert_costs(second, &[&format!("Mana({second_cost})"), "Tap"], id);
        assert_oracle_line(first, 1, id);
        assert_oracle_line(second, 2, id);
    }

    let armorer = face(
        "armorer_guildmage",
        "Armorer Guildmage",
        "{R}",
        &["Creature", "Human", "Wizard"],
    );
    assert!(matches!(
        activated(armorer, 0, "armorer_guildmage").effect.as_slice(),
        [SpellEffectKind::PumpTarget {
            power: 1,
            toughness: 0,
            subject: EffectSubject::Chosen(_),
            ..
        }]
    ));
    assert!(matches!(
        activated(armorer, 1, "armorer_guildmage").effect.as_slice(),
        [SpellEffectKind::PumpTarget {
            power: 0,
            toughness: 1,
            subject: EffectSubject::Chosen(_),
            ..
        }]
    ));

    let bant = face(
        "bant_battlemage",
        "Bant Battlemage",
        "{2}{W}",
        &["Creature", "Human", "Wizard"],
    );
    assert!(matches!(
        activated(bant, 0, "bant_battlemage").effect.as_slice(),
        [SpellEffectKind::GrantKeywords { subject: EffectSubject::Chosen(_), keywords }]
            if keywords == &[Keyword::Trample]
    ));
    assert!(matches!(
        activated(bant, 1, "bant_battlemage").effect.as_slice(),
        [SpellEffectKind::GrantKeywords { subject: EffectSubject::Chosen(_), keywords }]
            if keywords == &[Keyword::Flying]
    ));

    let naya = face(
        "naya_battlemage",
        "Naya Battlemage",
        "{2}{G}",
        &["Creature", "Human", "Shaman"],
    );
    assert!(matches!(
        activated(naya, 0, "naya_battlemage").effect.as_slice(),
        [SpellEffectKind::PumpTarget {
            power: 2,
            toughness: 0,
            subject: EffectSubject::Chosen(_),
            ..
        }]
    ));
    assert!(matches!(
        activated(naya, 1, "naya_battlemage").effect.as_slice(),
        [SpellEffectKind::Tap {
            subject: EffectSubject::Chosen(_)
        }]
    ));

    let senate = face(
        "senate_guildmage",
        "Senate Guildmage",
        "{W}{U}",
        &["Creature", "Human", "Wizard"],
    );
    assert!(matches!(
        activated(senate, 0, "senate_guildmage").effect.as_slice(),
        [SpellEffectKind::GainLife {
            amount: Amount::Fixed(2),
            ..
        }]
    ));
    assert!(activated(senate, 1, "senate_guildmage").effect.is_empty());
    assert!(
        activated(senate, 1, "senate_guildmage").program.is_some(),
        "Senate Guildmage draws before it discards"
    );

    let shaper = face(
        "shaper_guildmage",
        "Shaper Guildmage",
        "{U}",
        &["Creature", "Human", "Wizard"],
    );
    assert!(matches!(
        activated(shaper, 0, "shaper_guildmage").effect.as_slice(),
        [SpellEffectKind::GrantKeywords { subject: EffectSubject::Chosen(_), keywords }]
            if keywords == &[Keyword::FirstStrike]
    ));
    assert!(matches!(
        activated(shaper, 1, "shaper_guildmage").effect.as_slice(),
        [SpellEffectKind::PumpTarget {
            power: 1,
            toughness: 0,
            subject: EffectSubject::Chosen(_),
            ..
        }]
    ));

    // 03: a five-color tap mana ability on every card, plus one printed rider.
    for (id, name, mana, types, p, t) in [
        (
            "bronze_walrus",
            "Bronze Walrus",
            "{3}",
            &["Artifact", "Creature", "Walrus"][..],
            2u32,
            2u32,
        ),
        (
            "honored_heirloom",
            "Honored Heirloom",
            "{3}",
            &["Artifact"][..],
            0,
            0,
        ),
        (
            "letter_of_acceptance",
            "Letter of Acceptance",
            "{3}",
            &["Artifact"][..],
            0,
            0,
        ),
        ("mana_geode", "Mana Geode", "{3}", &["Artifact"][..], 0, 0),
        (
            "spinning_wheel",
            "Spinning Wheel",
            "{3}",
            &["Artifact"][..],
            0,
            0,
        ),
    ] {
        let f = face(id, name, mana, types);
        if p == 0 && t == 0 {
            assert_eq!((f.power, f.toughness), (None, None), "{id}");
        } else {
            assert_eq!((f.power, f.toughness), (Some(p), Some(t)), "{id}");
        }
        assert!(f.keywords.is_empty(), "{id}");
        let expected_abilities = if matches!(id, "bronze_walrus" | "mana_geode") {
            1
        } else {
            2
        };
        assert_eq!(f.activated_abilities.len(), expected_abilities, "{id}");
        assert_costs(activated(f, 0, id), &["Tap"], id);
        let mana_line = if matches!(id, "bronze_walrus" | "mana_geode") {
            2
        } else {
            1
        };
        assert_oracle_line(activated(f, 0, id), mana_line, id);
        assert_any_color_mana(f, 0, id);
    }

    let geode = face("mana_geode", "Mana Geode", "{3}", &["Artifact"]);
    assert_eq!(geode.triggered_abilities.len(), 1);
    assert_eq!(
        geode.triggered_abilities[0].trigger,
        TriggerCondition::WhenSelfEntersBattlefield
    );
    assert!(matches!(
        geode.triggered_abilities[0].effect.as_slice(),
        [SpellEffectKind::Scry {
            count: Amount::Fixed(1)
        }]
    ));
    assert_eq!(
        geode.triggered_abilities[0].presentation,
        AbilityPresentation::OracleLines(vec![1])
    );

    let walrus = face(
        "bronze_walrus",
        "Bronze Walrus",
        "{3}",
        &["Artifact", "Creature", "Walrus"],
    );
    assert_eq!(walrus.triggered_abilities.len(), 1);
    assert!(matches!(
        walrus.triggered_abilities[0].effect.as_slice(),
        [SpellEffectKind::Scry {
            count: Amount::Fixed(2)
        }]
    ));

    let wheel = face("spinning_wheel", "Spinning Wheel", "{3}", &["Artifact"]);
    assert_costs(
        activated(wheel, 1, "spinning_wheel"),
        &["Mana({5})", "Tap"],
        "spinning_wheel",
    );
    assert!(matches!(
        activated(wheel, 1, "spinning_wheel").effect.as_slice(),
        [SpellEffectKind::Tap {
            subject: EffectSubject::Chosen(_)
        }]
    ));

    let heirloom = face("honored_heirloom", "Honored Heirloom", "{3}", &["Artifact"]);
    assert_costs(
        activated(heirloom, 1, "honored_heirloom"),
        &["Mana({2})", "Tap"],
        "honored_heirloom",
    );
    assert!(matches!(
        activated(heirloom, 1, "honored_heirloom").effect.as_slice(),
        [SpellEffectKind::MoveGraveyardCards {
            destination: GraveyardDestination::Exile,
            ..
        }]
    ));

    let letter = face(
        "letter_of_acceptance",
        "Letter of Acceptance",
        "{3}",
        &["Artifact"],
    );
    assert_costs(
        activated(letter, 1, "letter_of_acceptance"),
        &["Mana({2})", "Tap", "SacrificeSelf"],
        "letter_of_acceptance",
    );
    assert!(matches!(
        activated(letter, 1, "letter_of_acceptance")
            .effect
            .as_slice(),
        [SpellEffectKind::Draw {
            count: Amount::Fixed(1),
            ..
        }]
    ));
}

#[test]
fn q69_batch_a_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, fingerprint) in [
        (
            "nights_whisper",
            "Night's Whisper",
            "c7731acf386b76aed6ba1453567b1d31bb9670c3416f85bdb5d5294ea90d0d8f",
        ),
        (
            "demons_due",
            "Demon's Due",
            "4d3dbabfa23e023ae9f9ffbde692f5fecb014bb69a6464f029b889f1d4d2dec1",
        ),
        (
            "read_the_bones",
            "Read the Bones",
            "8a20f9fa36222fd79e2056ab9065b8b819a19e14f2c942e50feffffe4b255fd1",
        ),
        (
            "armorer_guildmage",
            "Armorer Guildmage",
            "43cd2f89a19296460cb2a82920c8c8ba25def998add074bfe34b860eb159d02b",
        ),
        (
            "bant_battlemage",
            "Bant Battlemage",
            "5104f07912b830d1b1789769deff86e826daf23c544169b62ab96412b22238bf",
        ),
        (
            "naya_battlemage",
            "Naya Battlemage",
            "950c66dcf1b1d793610d6e3d75a57e22a05f1cdeeea15870a1604d3ffc86ba73",
        ),
        (
            "senate_guildmage",
            "Senate Guildmage",
            "ffa607efb7c00ba2c2fceb407a282b9e968b1ceecb1ff6b386e4c84757cce2aa",
        ),
        (
            "shaper_guildmage",
            "Shaper Guildmage",
            "ab2d4338e9847c239ccf4aa6cd650b9795fd7b574fe8677e6ea5f13ae85e53c4",
        ),
        (
            "bronze_walrus",
            "Bronze Walrus",
            "94a60a578c275cf6cfdbf965df747d264f5c972f41aab5a9806e6ba0353d5eae",
        ),
        (
            "honored_heirloom",
            "Honored Heirloom",
            "d303ed36d5ead10c1280074e14e285db656342178de2b022157cf60163dfc427",
        ),
        (
            "letter_of_acceptance",
            "Letter of Acceptance",
            "c04fe93e2998ec94681eb1dfde54189845d6dac866cbfb304f34672b8adcd28d",
        ),
        (
            "mana_geode",
            "Mana Geode",
            "e17a72d4107b2d058525ce23e8b5596c03a44f592074fd0d64f7ad7cbf0a9e75",
        ),
        (
            "spinning_wheel",
            "Spinning Wheel",
            "53707a58b8b40b7b6d87df80aeb6f064740640712d71ba0b380fceec14b52ced",
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
