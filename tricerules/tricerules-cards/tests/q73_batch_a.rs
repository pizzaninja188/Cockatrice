//! Registry conformance for the queue-v73 batch A cohorts.
//!
//! Three stable typed shapes: a card-advantage spell body plus hand-zone cycling, a control spell
//! body plus a printed flashback cost, and a simple spell body plus the Madness static. Fingerprints
//! are added after the presentation metadata refresh.

use tricerules_cards::primitives::{
    Amount, EffectSubject, Keyword, SpellEffectKind, StaticAbilityDef,
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

fn assert_cycling(f: &CardFace, cost: &str, line: u16, id: &str) {
    let ability = f
        .activated_abilities
        .first()
        .unwrap_or_else(|| panic!("{id}: missing cycling ability"));
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![line]),
        "{id}: cycling presentation"
    );
    assert_eq!(ability.source_zone, AbilitySourceZone::Hand, "{id}");
    assert!(
        matches!(
            ability.costs.as_slice(),
            [AbilityCost::Mana(mana), AbilityCost::DiscardSelf]
                if mana.to_string() == cost
        ),
        "{id}: cycling cost {cost}"
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

fn assert_madness(f: &CardFace, cost: &str, id: &str) {
    let static_ability = f
        .static_abilities
        .first()
        .unwrap_or_else(|| panic!("{id}: missing madness"));
    assert_eq!(
        static_ability.presentation,
        AbilityPresentation::OracleLines(vec![2]),
        "{id}: madness presentation"
    );
    assert!(
        matches!(
            &static_ability.definition,
            StaticAbilityDef::Madness { cost: actual } if actual.to_string() == cost
        ),
        "{id}: madness cost {cost}"
    );
    assert_eq!(f.static_abilities.len(), 1, "{id}: one static ability");
}

#[test]
fn q73_cycling_card_advantage_map_definitions() {
    for (id, name, mana, cost, types) in [
        (
            "hieroglyphic_illumination",
            "Hieroglyphic Illumination",
            "{3}{U}",
            "{U}",
            &["Instant"][..],
        ),
        (
            "boon_of_the_wish-giver",
            "Boon of the Wish-Giver",
            "{4}{U}{U}",
            "{1}",
            &["Sorcery"][..],
        ),
        (
            "compelling_argument",
            "Compelling Argument",
            "{1}{U}",
            "{U}",
            &["Sorcery"][..],
        ),
        (
            "rejuvenate",
            "Rejuvenate",
            "{3}{G}",
            "{2}",
            &["Sorcery"][..],
        ),
    ] {
        let f = face(id, name, mana, types);
        assert!(f.spell_keywords.is_empty(), "{id}: no spell keyword");
        assert!(f.flashback_cost.is_none(), "{id}: no flashback");
        assert!(f.static_abilities.is_empty(), "{id}: no static ability");
        assert_cycling(f, cost, 2, id);
    }

    let illumination = face(
        "hieroglyphic_illumination",
        "Hieroglyphic Illumination",
        "{3}{U}",
        &["Instant"],
    );
    assert!(illumination.targeting.is_none(), "no target");
    assert!(
        matches!(
            illumination.spell_effect.as_slice(),
            [SpellEffectKind::Draw {
                count: Amount::Fixed(2),
                ..
            }]
        ),
        "Hieroglyphic Illumination draws two"
    );

    let boon = face(
        "boon_of_the_wish-giver",
        "Boon of the Wish-Giver",
        "{4}{U}{U}",
        &["Sorcery"],
    );
    assert!(boon.targeting.is_none(), "no target");
    assert!(
        format!("{:?}", boon.spell_effect).contains("Draw")
            && format!("{:?}", boon.spell_effect).contains("4"),
        "Boon of the Wish-Giver draws four"
    );

    let argument = face(
        "compelling_argument",
        "Compelling Argument",
        "{1}{U}",
        &["Sorcery"],
    );
    assert!(
        format!("{:?}", argument.spell_effect).contains("MillTargetPlayer")
            && format!("{:?}", argument.spell_effect).contains("AnyPlayer")
            && format!("{:?}", argument.spell_effect).contains("5"),
        "Compelling Argument mills any chosen player for five"
    );

    let rejuvenate = face("rejuvenate", "Rejuvenate", "{3}{G}", &["Sorcery"]);
    assert!(
        format!("{:?}", rejuvenate.spell_effect).contains("GainLife")
            && format!("{:?}", rejuvenate.spell_effect).contains("6"),
        "Rejuvenate gains six life"
    );
}

#[test]
fn q73_flashback_control_map_definitions() {
    for (id, name, mana, flashback, types) in [
        (
            "chainers_edict",
            "Chainer's Edict",
            "{1}{B}",
            "{5}{B}{B}",
            &["Sorcery"][..],
        ),
        (
            "dematerialize",
            "Dematerialize",
            "{3}{U}",
            "{5}{U}{U}",
            &["Sorcery"][..],
        ),
        (
            "silent_departure",
            "Silent Departure",
            "{U}",
            "{4}{U}",
            &["Sorcery"][..],
        ),
        (
            "rolling_temblor",
            "Rolling Temblor",
            "{2}{R}",
            "{4}{R}{R}",
            &["Sorcery"][..],
        ),
    ] {
        let f = face(id, name, mana, types);
        assert_eq!(
            f.flashback_cost.as_ref().map(ToString::to_string),
            Some(flashback.to_string()),
            "{id}"
        );
        assert!(f.spell_keywords.is_empty(), "{id}: no spell keyword");
        assert!(
            f.activated_abilities.is_empty(),
            "{id}: no activated ability"
        );
        assert!(f.static_abilities.is_empty(), "{id}: no static ability");
    }

    let edict = face("chainers_edict", "Chainer's Edict", "{1}{B}", &["Sorcery"]);
    assert!(
        format!("{:?}", edict.spell_effect).contains("TargetPlayerSacrifices")
            && format!("{:?}", edict.spell_effect).contains("AnyPlayer"),
        "Chainer's Edict is a player-scoped edict"
    );

    let dematerialize = face("dematerialize", "Dematerialize", "{3}{U}", &["Sorcery"]);
    assert!(
        format!("{:?}", dematerialize.spell_effect).contains("ReturnToOwnersHand")
            && format!("{:?}", dematerialize.spell_effect).contains("AnyPermanent"),
        "Dematerialize returns any permanent"
    );

    let departure = face("silent_departure", "Silent Departure", "{U}", &["Sorcery"]);
    assert!(
        format!("{:?}", departure.spell_effect).contains("ReturnToOwnersHand")
            && format!("{:?}", departure.spell_effect).contains("Creature"),
        "Silent Departure returns a creature"
    );

    let temblor = face("rolling_temblor", "Rolling Temblor", "{2}{R}", &["Sorcery"]);
    assert!(
        format!("{:?}", temblor.spell_effect).contains("DamageAll")
            && format!("{:?}", temblor.spell_effect).contains("2")
            && format!("{:?}", temblor.spell_effect).contains("Flying"),
        "Rolling Temblor excludes flying creatures"
    );
}

#[test]
fn q73_madness_spells_map_definitions() {
    for (id, name, mana, madness, types) in [
        (
            "dark_withering",
            "Dark Withering",
            "{4}{B}{B}",
            "{B}",
            &["Instant"][..],
        ),
        (
            "distemper_of_the_blood",
            "Distemper of the Blood",
            "{1}{R}",
            "{R}",
            &["Sorcery"][..],
        ),
        (
            "frantic_purification",
            "Frantic Purification",
            "{2}{W}",
            "{W}",
            &["Instant"][..],
        ),
        (
            "just_the_wind",
            "Just the Wind",
            "{1}{U}",
            "{U}",
            &["Instant"][..],
        ),
        (
            "obsessive_search",
            "Obsessive Search",
            "{U}",
            "{U}",
            &["Instant"][..],
        ),
    ] {
        let f = face(id, name, mana, types);
        assert!(f.spell_keywords.is_empty(), "{id}: no spell keyword");
        assert!(f.flashback_cost.is_none(), "{id}: no flashback");
        assert!(
            f.activated_abilities.is_empty(),
            "{id}: no activated ability"
        );
        assert_madness(f, madness, id);
    }

    let distemper = face(
        "distemper_of_the_blood",
        "Distemper of the Blood",
        "{1}{R}",
        &["Sorcery"],
    );
    let targeting = distemper.targeting.as_ref().expect("targeting");
    let group = targeting.groups.first().expect("target group");
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(group.effect_indices, [0, 1]);
    assert!(
        matches!(
            distemper.spell_effect.as_slice(),
            [
                SpellEffectKind::PumpTarget {
                    power: 2,
                    toughness: 2,
                    ..
                },
                SpellEffectKind::GrantKeywords {
                    subject: EffectSubject::Chosen(_),
                    keywords,
                }
            ] if keywords == &[Keyword::Trample]
        ),
        "Distemper of the Blood pumps and grants trample"
    );

    let withering = face(
        "dark_withering",
        "Dark Withering",
        "{4}{B}{B}",
        &["Instant"],
    );
    assert!(
        format!("{:?}", withering.spell_effect).contains("Destroy")
            && format!("{:?}", withering.spell_effect).contains("Black"),
        "Dark Withering destroys a nonblack creature"
    );
    assert!(
        withering
            .targeting
            .as_ref()
            .is_some_and(|t| t.groups.len() == 1),
        "Dark Withering has one target group"
    );

    let purification = face(
        "frantic_purification",
        "Frantic Purification",
        "{2}{W}",
        &["Instant"],
    );
    assert!(
        format!("{:?}", purification.spell_effect).contains("Destroy")
            && format!("{:?}", purification.spell_effect).contains("Enchantment"),
        "Frantic Purification destroys an enchantment"
    );

    let wind = face("just_the_wind", "Just the Wind", "{1}{U}", &["Instant"]);
    assert!(
        format!("{:?}", wind.spell_effect).contains("ReturnToOwnersHand")
            && format!("{:?}", wind.spell_effect).contains("Creature"),
        "Just the Wind returns a creature"
    );

    let search = face("obsessive_search", "Obsessive Search", "{U}", &["Instant"]);
    assert!(
        matches!(
            search.spell_effect.as_slice(),
            [SpellEffectKind::Draw {
                count: Amount::Fixed(1),
                ..
            }]
        ),
        "Obsessive Search draws one"
    );
    assert!(search.targeting.is_none(), "Obsessive Search has no target");
}

#[test]
fn q73_batch_a_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, face_id, fingerprint) in [
        (
            "hieroglyphic_illumination",
            "Hieroglyphic Illumination",
            "hieroglyphic_illumination",
            "3adf96f9768abd24f8043ce73c7898108cc0e844b79b5a36f44d503d4108174d",
        ),
        (
            "boon_of_the_wish-giver",
            "Boon of the Wish-Giver",
            "boon_of_the_wish_giver",
            "8a4da8b44c05243e5f3036ca50e74c8f8113e5b88ec14cff3c0ec20de9ac6abb",
        ),
        (
            "compelling_argument",
            "Compelling Argument",
            "compelling_argument",
            "ffa71bbd6c3a6f339111d3102041bf229d68563c4e7739d801a078fd08a48411",
        ),
        (
            "rejuvenate",
            "Rejuvenate",
            "rejuvenate",
            "5c47abd6a64bb1a41477c79b1642fb365cd6349ab344f0f6576a70063c872119",
        ),
        (
            "chainers_edict",
            "Chainer's Edict",
            "chainers_edict",
            "b3927821451bdc143f2778421fe72c361c064ec33d0e2274ac095d8b3ad9fcc4",
        ),
        (
            "dematerialize",
            "Dematerialize",
            "dematerialize",
            "7ed059c0993577172379726c1dc44d6b2420bb05c0d4d33905194b171f472a43",
        ),
        (
            "silent_departure",
            "Silent Departure",
            "silent_departure",
            "d158d93752367ea7990353b9bb664f7a5e05c1b8ee78d784505e68d20a53821e",
        ),
        (
            "rolling_temblor",
            "Rolling Temblor",
            "rolling_temblor",
            "56126215d6e897b13bfda531a621d4c0736b88ed959bb33384fb49ecbe026eab",
        ),
        (
            "dark_withering",
            "Dark Withering",
            "dark_withering",
            "0c7963f8d6a91306b9dfa955a62bfabaa7ac09cf653e02a2f082aa0dbbba261b",
        ),
        (
            "distemper_of_the_blood",
            "Distemper of the Blood",
            "distemper_of_the_blood",
            "39cf3c6ec505802c48bce720d18605a0e537ce24a4a2e7d09100a0125d79f33c",
        ),
        (
            "frantic_purification",
            "Frantic Purification",
            "frantic_purification",
            "ee1a0bceded0bc71ccb446b81d1fb51409482633a4d9647f47c4c0b103fa334e",
        ),
        (
            "just_the_wind",
            "Just the Wind",
            "just_the_wind",
            "d52efb7774377efb2bc45c406a12d0fe4fbec1cbc9fdfb6f4b2bcc5eac307679",
        ),
        (
            "obsessive_search",
            "Obsessive Search",
            "obsessive_search",
            "6a96895c5b109c5a1eb75847df17624676a720fdfcac10d69870daf0490a1b35",
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
        assert_eq!(fields[2], face_id, "face id drift for {id}");
        assert_eq!(fields[3], name, "display name drift for {id}");
        assert_eq!(fields[4], fingerprint, "pinned Oracle fingerprint for {id}");
    }
}
