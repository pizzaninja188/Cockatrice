//! Registry conformance for the queue-v72 batch A cohorts.
//!
//! Three stable typed shapes: an Aura with one attached P/T (and optional keyword) modifier plus
//! hand-zone cycling, a combat pump or keyword grant plus hand-zone cycling, and a pump/targeted
//! effect or basic-land search plus investigate. Fingerprints are added after the presentation
//! metadata refresh.

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

fn assert_attached_modifier(
    f: &CardFace,
    power: i32,
    toughness: i32,
    keywords: &[Keyword],
    id: &str,
) {
    let static_ability = f
        .static_abilities
        .first()
        .unwrap_or_else(|| panic!("{id}: attached modifier"));
    assert!(
        matches!(
            &static_ability.definition,
            StaticAbilityDef::AttachedModifier {
                delta_power,
                delta_toughness,
                keywords: actual,
                ..
            } if *delta_power == power && *delta_toughness == toughness && actual == keywords
        ),
        "{id}: attached modifier {power}/{toughness} {keywords:?}"
    );
    assert_eq!(
        static_ability.presentation,
        AbilityPresentation::OracleLines(vec![2]),
        "{id}: modifier presentation"
    );
}

#[test]
fn q72_cycling_auras_map_definitions() {
    for (id, name, mana, power, toughness, keywords, cost) in [
        (
            "improvised_armor",
            "Improvised Armor",
            "{3}{W}",
            2,
            5,
            &[][..],
            "{3}",
        ),
        (
            "savage_hunger",
            "Savage Hunger",
            "{2}{G}",
            1,
            0,
            &[Keyword::Trample][..],
            "{2}",
        ),
        ("sicken", "Sicken", "{B}", -1, -1, &[][..], "{2}"),
    ] {
        let f = face(id, name, mana, &["Enchantment", "Aura"]);
        assert!(f.spell_keywords.is_empty(), "{id}: no spell keyword");
        assert!(f.flashback_cost.is_none(), "{id}: no flashback");
        assert!(
            matches!(
                f.spell_effect.as_slice(),
                [SpellEffectKind::AuraAttach { .. }]
            ),
            "{id}: AuraAttach"
        );
        assert!(f.targeting.is_none(), "{id}: AuraAttach owns its target");
        assert_attached_modifier(f, power, toughness, keywords, id);
        assert_cycling(f, cost, 3, id);
    }
}

#[test]
fn q72_cycling_combat_map_definitions() {
    for (id, name, mana, cost) in [
        ("coordinated_charge", "Coordinated Charge", "{4}{W}", "{2}"),
        ("pursue_glory", "Pursue Glory", "{3}{R}", "{2}"),
        ("raking_claws", "Raking Claws", "{1}{R}", "{2}"),
    ] {
        let f = face(id, name, mana, &["Instant"]);
        assert!(f.spell_keywords.is_empty(), "{id}: no spell keyword");
        assert!(f.flashback_cost.is_none(), "{id}: no flashback");
        assert_cycling(f, cost, 2, id);
    }

    let coordinated = face(
        "coordinated_charge",
        "Coordinated Charge",
        "{4}{W}",
        &["Instant"],
    );
    assert!(
        coordinated.targeting.is_none(),
        "Coordinated Charge: untargeted"
    );
    assert!(
        matches!(
            coordinated.spell_effect.as_slice(),
            [SpellEffectKind::PumpAll {
                power: 2,
                toughness: 1,
                ..
            }]
        ),
        "Coordinated Charge pump"
    );
    assert!(
        format!("{:?}", coordinated.spell_effect).contains("YouControl"),
        "Coordinated Charge only your creatures"
    );

    let pursue = face("pursue_glory", "Pursue Glory", "{3}{R}", &["Instant"]);
    assert!(pursue.targeting.is_none(), "Pursue Glory: untargeted");
    assert!(
        matches!(
            pursue.spell_effect.as_slice(),
            [SpellEffectKind::PumpAll {
                power: 2,
                toughness: 0,
                ..
            }]
        ),
        "Pursue Glory pump"
    );
    assert!(
        format!("{:?}", pursue.spell_effect).contains("combat_role: Some(Attacking)"),
        "Pursue Glory only attacking creatures"
    );

    let raking = face("raking_claws", "Raking Claws", "{1}{R}", &["Instant"]);
    assert_target_group(raking, 1, 1, &[0], "raking_claws");
    assert!(
        matches!(
            raking.spell_effect.as_slice(),
            [SpellEffectKind::GrantKeywords {
                subject: EffectSubject::Chosen(_),
                keywords,
            }] if keywords == &[Keyword::DoubleStrike]
        ),
        "Raking Claws double strike"
    );
}

#[test]
fn q72_investigate_riders_map_definitions() {
    // Survive the Night: the pump and keyword share the only target; investigate is untargeted.
    let survive = face(
        "survive_the_night",
        "Survive the Night",
        "{2}{W}",
        &["Instant"],
    );
    assert_target_group(survive, 1, 1, &[0, 1], "survive_the_night");
    assert!(
        matches!(
            survive.spell_effect.as_slice(),
            [
                SpellEffectKind::PumpTarget {
                    power: 1,
                    toughness: 0,
                    ..
                },
                SpellEffectKind::GrantKeywords {
                    subject: EffectSubject::Chosen(_),
                    keywords,
                },
                SpellEffectKind::CreateTokens { token, count, .. }
            ] if keywords == &[Keyword::Indestructible]
                && token == "clue"
                && *count == Amount::Fixed(1)
        ),
        "Survive the Night pump, indestructible and Clue"
    );

    // The Chase Is On: one line, same target group for pump and first strike.
    let chase = face("the_chase_is_on", "The Chase Is On", "{2}{R}", &["Instant"]);
    assert_target_group(chase, 1, 1, &[0, 1], "the_chase_is_on");
    assert!(
        matches!(
            chase.spell_effect.as_slice(),
            [
                SpellEffectKind::PumpTarget {
                    power: 3,
                    toughness: 0,
                    ..
                },
                SpellEffectKind::GrantKeywords {
                    subject: EffectSubject::Chosen(_),
                    keywords,
                },
                SpellEffectKind::Investigate { count: Amount::Fixed(1), .. }
            ] if keywords == &[Keyword::FirstStrike]
        ),
        "The Chase Is On pump, first strike and Clue"
    );

    // They Went This Way: search a basic land onto the battlefield tapped, then investigate.
    let went = face(
        "they_went_this_way",
        "They Went This Way",
        "{2}{G}",
        &["Sorcery"],
    );
    assert!(went.targeting.is_none(), "They Went This Way: untargeted");
    assert!(
        matches!(
            went.spell_effect.as_slice(),
            [
                SpellEffectKind::SearchLibrary {
                    filter: Some(_),
                    destination: tricerules_cards::primitives::SearchDestination::Battlefield {
                        tapped: true
                    },
                    ..
                },
                SpellEffectKind::Investigate {
                    count: Amount::Fixed(1),
                    ..
                }
            ]
        ),
        "They Went This Way search and Clue"
    );
    assert!(
        format!("{:?}", went.spell_effect).contains("BasicLand"),
        "They Went This Way searches only a basic land"
    );
}

#[test]
fn q72_batch_a_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, fingerprint) in [
        (
            "improvised_armor",
            "Improvised Armor",
            "46bcb97a27b97abd4d1aa234beece905f70654ae81371b7876c6288868241e6d",
        ),
        (
            "savage_hunger",
            "Savage Hunger",
            "3f94ce25f142acb2e7d02d01acd4bfb5f1f8daeba57ac36ea69bdd2a590053a0",
        ),
        (
            "sicken",
            "Sicken",
            "e0d938240c854415532e3bd15b4e00b5af70eb094bce6f0adf47c773618e50bb",
        ),
        (
            "coordinated_charge",
            "Coordinated Charge",
            "4bc13dbbeef092d5a8bd71f2c11d6e3755d2657cb52407c3a5432423b4bb2d1b",
        ),
        (
            "pursue_glory",
            "Pursue Glory",
            "b2307b9c8fde0417f9632cc25f783ff9441a70a0b3594a642c6ae8568258eaf8",
        ),
        (
            "raking_claws",
            "Raking Claws",
            "ff60bfe83959daea90e04112be9bf850ab1d7ebb8f844eb2540be7e7b184e24e",
        ),
        (
            "survive_the_night",
            "Survive the Night",
            "7aad83e645765efdc8cc2a5b12098b3cdec0151a45b6a126417e006e1a281806",
        ),
        (
            "the_chase_is_on",
            "The Chase Is On",
            "2c6eccc20d4efcca96d79adc617163b43b47ba849a02ecff35610aaa34c0fa89",
        ),
        (
            "they_went_this_way",
            "They Went This Way",
            "1db088416f97cb414bb3683c1b2555d5370ce683799f6fcd98fdd5df5aa9baf5",
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
