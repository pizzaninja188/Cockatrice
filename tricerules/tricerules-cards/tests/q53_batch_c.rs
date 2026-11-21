//! Registry conformance for the queue-v53 batch C cohorts.
#![allow(clippy::type_complexity)]
use tricerules_cards::primitives::{AbilityCost, Evasion, SpellEffectKind};
use tricerules_cards::{registry, AbilityPresentation, CardFace, Layout};

fn identity(id: &str, name: &str, mana: &str, types: &[&str]) -> &'static CardFace {
    let definition = registry::global()
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"));
    assert_eq!(definition.layout, Layout::Normal, "{id}");
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

fn assert_keywords(f: &CardFace, id: &str, expected: &[&str]) {
    assert_eq!(
        f.keywords
            .iter()
            .map(|k| format!("{k:?}"))
            .collect::<Vec<_>>(),
        expected,
        "{id} keywords"
    );
}

fn assert_presentation(ability: &tricerules_cards::primitives::ActivatedAbilityDef, id: &str) {
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![2]),
        "{id}: printed ability line"
    );
}

#[test]
fn q53_regenerate_cards_map_definitions() {
    for (id, name, mana, types, power, toughness, keyword, cost) in [
        (
            "drudge_reavers",
            "Drudge Reavers",
            "{3}{B}",
            &["Creature", "Skeleton"][..],
            2,
            1,
            "Flash",
            "{B}",
        ),
        (
            "ghost_ship",
            "Ghost Ship",
            "{2}{U}{U}",
            &["Creature", "Spirit"][..],
            2,
            4,
            "Flying",
            "{U}{U}{U}",
        ),
        (
            "malach_of_the_dawn",
            "Malach of the Dawn",
            "{2}{W}{W}",
            &["Creature", "Angel"][..],
            2,
            4,
            "Flying",
            "{W}{W}{W}",
        ),
        (
            "ranger_en-vec",
            "Ranger en-Vec",
            "{1}{G}{W}",
            &["Creature", "Human", "Soldier", "Archer", "Ranger"][..],
            2,
            2,
            "FirstStrike",
            "{G}",
        ),
        (
            "troll_ascetic",
            "Troll Ascetic",
            "{1}{G}{G}",
            &["Creature", "Troll", "Shaman"][..],
            3,
            2,
            "Hexproof",
            "{1}{G}",
        ),
        (
            "cinderbones",
            "Cinderbones",
            "{2}{B}",
            &["Creature", "Elemental", "Skeleton"][..],
            1,
            1,
            "Wither",
            "{1}{B}",
        ),
    ] {
        let f = identity(id, name, mana, types);
        assert_eq!(
            (f.power, f.toughness),
            (Some(power), Some(toughness)),
            "{id} P/T"
        );
        assert_keywords(f, id, &[keyword]);
        let [ability] = f.activated_abilities.as_slice() else {
            panic!("{id}: one ability")
        };
        assert_presentation(ability, id);
        assert!(
            matches!(&ability.costs[..], [AbilityCost::Mana(actual)] if actual.to_string() == cost),
            "{id}: exact mana"
        );
        assert!(
            matches!(
                ability.effect.as_slice(),
                [SpellEffectKind::Regenerate { .. }]
            ),
            "{id}: regenerate"
        );
    }
}

#[test]
fn q53_self_pump_cards_map_definitions() {
    for (id, name, mana, types, power, toughness, dp, dt, keywords, evasion, cost) in [
        (
            "whispering_shade",
            "Whispering Shade",
            "{3}{B}",
            &["Creature", "Shade"][..],
            1,
            1,
            1,
            1,
            &[][..],
            Some("Swamp"),
            "{B}",
        ),
        (
            "dirtwater_wraith",
            "Dirtwater Wraith",
            "{3}{B}",
            &["Creature", "Wraith"][..],
            1,
            3,
            1,
            0,
            &[][..],
            Some("Swamp"),
            "{B}",
        ),
        (
            "colos_yearling",
            "Colos Yearling",
            "{2}{R}",
            &["Creature", "Goat", "Beast"][..],
            1,
            1,
            1,
            0,
            &[][..],
            Some("Mountain"),
            "{R}",
        ),
        (
            "sandstone_warrior",
            "Sandstone Warrior",
            "{2}{R}{R}",
            &["Creature", "Human", "Soldier", "Warrior"][..],
            1,
            3,
            1,
            0,
            &["FirstStrike"][..],
            None,
            "{R}",
        ),
        (
            "loxodon_stalwart",
            "Loxodon Stalwart",
            "{3}{W}{W}",
            &["Creature", "Elephant", "Soldier"][..],
            3,
            3,
            0,
            1,
            &["Vigilance"][..],
            None,
            "{W}",
        ),
        (
            "steam_spitter",
            "Steam Spitter",
            "{4}{G}",
            &["Creature", "Spider"][..],
            1,
            5,
            1,
            0,
            &["Reach"][..],
            None,
            "{R}",
        ),
    ] {
        let f = identity(id, name, mana, types);
        assert_eq!(
            (f.power, f.toughness),
            (Some(power), Some(toughness)),
            "{id} P/T"
        );
        assert_keywords(f, id, keywords);
        match evasion {
            Some(subtype) => {
                assert_eq!(f.evasions.len(), 1, "{id}: one evasion");
                assert!(
                    matches!(&f.evasions[0], Evasion::Landwalk { land_subtype } if land_subtype == subtype),
                    "{id}: landwalk"
                );
            }
            None => assert!(f.evasions.is_empty(), "{id}: no evasion"),
        }
        let [ability] = f.activated_abilities.as_slice() else {
            panic!("{id}: one ability")
        };
        assert_presentation(ability, id);
        assert!(
            matches!(&ability.costs[..], [AbilityCost::Mana(actual)] if actual.to_string() == cost),
            "{id}: exact pump cost"
        );
        assert!(
            matches!(
                ability.effect.as_slice(),
                [SpellEffectKind::PumpTarget { power: p, toughness: t, subject, .. }]
                    if *p == dp && *t == dt && matches!(subject, tricerules_cards::primitives::EffectSubject::Source)
            ),
            "{id}: printed pump"
        );
    }
}

#[test]
fn q53_any_color_mana_cards_map_definitions() {
    for (id, name, mana, types, power, toughness, keywords) in [
        (
            "deathbloom_gardener",
            "Deathbloom Gardener",
            "{2}{G}",
            &["Creature", "Elf", "Druid"][..],
            Some(1),
            Some(1),
            &["Deathtouch"][..],
        ),
        (
            "sylvan_caryatid",
            "Sylvan Caryatid",
            "{1}{G}",
            &["Creature", "Plant"][..],
            Some(0),
            Some(3),
            &["Defender", "Hexproof"][..],
        ),
        (
            "leyline_prowler",
            "Leyline Prowler",
            "{1}{B}{G}",
            &["Creature", "Nightmare", "Beast"][..],
            Some(2),
            Some(3),
            &["Deathtouch", "Lifelink"][..],
        ),
        (
            "darksteel_ingot",
            "Darksteel Ingot",
            "{3}",
            &["Artifact"][..],
            None,
            None,
            &["Indestructible"][..],
        ),
    ] {
        let f = identity(id, name, mana, types);
        assert_eq!((f.power, f.toughness), (power, toughness), "{id} P/T");
        assert_keywords(f, id, keywords);
        let [ability] = f.activated_abilities.as_slice() else {
            panic!("{id}: one ability")
        };
        assert_presentation(ability, id);
        assert!(
            matches!(&ability.costs[..], [AbilityCost::Tap]),
            "{id}: tap cost"
        );
        let [SpellEffectKind::ProduceMana { options, .. }] = ability.effect.as_slice() else {
            panic!("{id}: produce mana")
        };
        assert_eq!(options.len(), 5, "{id}: five colors");
    }
}

fn fingerprint_face_id(name: &str) -> String {
    name.to_lowercase()
        .replace([' ', '-'], "_")
        .replace('\'', "")
}

#[test]
fn q53_c_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, face, digest) in [
        (
            "cinderbones",
            "Cinderbones",
            "cinderbones",
            "c2de86bc005863b2895d99c4fe4a9fb1f7336fcd42e2063d7e3ae68f80f9aee4",
        ),
        (
            "colos_yearling",
            "Colos Yearling",
            "colos_yearling",
            "0caf2745bbf84bc38d0c9a4c2cb697b5d8eb5abe4bcfb14d5524cb8e6be0af64",
        ),
        (
            "darksteel_ingot",
            "Darksteel Ingot",
            "darksteel_ingot",
            "88df9def48eb6c712a6bc92f6681adb1529a004dfa915b0063a881a39f8d42a3",
        ),
        (
            "deathbloom_gardener",
            "Deathbloom Gardener",
            "deathbloom_gardener",
            "87bd4dab6e908424821fd39b2b90a3815d0586baeefacc1ebc314e48f441e6b0",
        ),
        (
            "dirtwater_wraith",
            "Dirtwater Wraith",
            "dirtwater_wraith",
            "5d36f6f6745feb6c77f5561978b3fa7427179f75f7a523a8dc593b9fafefd496",
        ),
        (
            "drudge_reavers",
            "Drudge Reavers",
            "drudge_reavers",
            "c90355ba2ec954823dbf3e281e7ad549b85d4a1dd261976952e0085ecbc2801e",
        ),
        (
            "ghost_ship",
            "Ghost Ship",
            "ghost_ship",
            "b51189fa5ef7a752a5d9bd6677cacc67245331de0eeb0c6855c98b8349a0b03c",
        ),
        (
            "leyline_prowler",
            "Leyline Prowler",
            "leyline_prowler",
            "c389437504d9775f4345ae588606cccb95bddd72f41a9a136ab536d77005686d",
        ),
        (
            "loxodon_stalwart",
            "Loxodon Stalwart",
            "loxodon_stalwart",
            "5df85293fa5f8258ab82f982ed1318087217a9e4596d65070e7205ff35380592",
        ),
        (
            "malach_of_the_dawn",
            "Malach of the Dawn",
            "malach_of_the_dawn",
            "f5777f9adcfbfa45a6b9f162075772e9d94c6f2de13db0d8afba262391e2b850",
        ),
        (
            "ranger_en-vec",
            "Ranger en-Vec",
            "ranger_en_vec",
            "2cd3d9c269793c38f448a580eeae2e51c82abc89bab0b64c6d112fb3241f4897",
        ),
        (
            "sandstone_warrior",
            "Sandstone Warrior",
            "sandstone_warrior",
            "e96ca8be12dca0de9da4868930d46e47037b59df8c34331e16e4622bf4e5c4d8",
        ),
        (
            "steam_spitter",
            "Steam Spitter",
            "steam_spitter",
            "be2e4a027d60897ebf79c6ac8147c37e9f94479ab571f831d7a6e2447aa0579f",
        ),
        (
            "sylvan_caryatid",
            "Sylvan Caryatid",
            "sylvan_caryatid",
            "1ff64c9531b0439b8d4ea6b2baa60011503e527ce57316c3feedb85810827bf3",
        ),
        (
            "troll_ascetic",
            "Troll Ascetic",
            "troll_ascetic",
            "caa2c1804fb0acecf4d3b9510898515660082f53073c61b0065cffc79f1e4c33",
        ),
        (
            "whispering_shade",
            "Whispering Shade",
            "whispering_shade",
            "0a51e7c5e59eb79a1da89631eb044e32bb7a06bd9ce143339651d3f31eccbfe7",
        ),
    ] {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "{id}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(
            fields[2],
            face,
            "face id drift for {id} (helper {})",
            fingerprint_face_id(name)
        );
        assert_eq!(fields[3], name);
        assert_eq!(fields[4], digest, "pinned fingerprint digest for {id}");
    }
}
