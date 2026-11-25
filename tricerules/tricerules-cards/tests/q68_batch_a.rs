//! Registry conformance for the queue-v68 batch A cohorts.
//!
//! Three stable typed shapes: a one-mana 1/1 with two off-color `{C}, {T}` activated abilities, a
//! private top-library look that keeps one card and bins the rest, and an Aura whose own activated
//! ability returns it to its owner's hand. Fingerprints are asserted once the presentation metadata
//! is refreshed from the pinned Oracle text.

use tricerules_cards::primitives::{
    EffectSubject, SpellEffectKind, StaticAbilityDef, TargetKind, TriggerCondition,
};
use tricerules_cards::{registry, AbilityCost, AbilityPresentation, CardFace, Layout};

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
    let ability = f
        .activated_abilities
        .get(index)
        .unwrap_or_else(|| panic!("{id}: missing activated ability {index}"));
    ability
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
            other => format!("{other:?}"),
        })
        .collect();
    assert_eq!(actual, expected, "{id}");
}

#[test]
fn q68_batch_a_cohorts_map_definitions() {
    // 01: each Disciple is a printed 1/1 with exactly two off-color mana-and-tap abilities.
    for (id, name, mana, types, first_cost, second_cost) in [
        (
            "ana_disciple",
            "Ana Disciple",
            "{G}",
            &["Creature", "Human", "Wizard"][..],
            "{U}",
            "{B}",
        ),
        (
            "ceta_disciple",
            "Ceta Disciple",
            "{U}",
            &["Creature", "Merfolk", "Wizard"][..],
            "{R}",
            "{G}",
        ),
        (
            "dega_disciple",
            "Dega Disciple",
            "{W}",
            &["Creature", "Human", "Wizard"][..],
            "{B}",
            "{R}",
        ),
        (
            "necra_disciple",
            "Necra Disciple",
            "{B}",
            &["Creature", "Human", "Wizard"][..],
            "{G}",
            "{W}",
        ),
        (
            "raka_disciple",
            "Raka Disciple",
            "{R}",
            &["Creature", "Minotaur", "Wizard"][..],
            "{W}",
            "{U}",
        ),
    ] {
        let f = face(id, name, mana, types);
        assert_eq!((f.power, f.toughness), (Some(1), Some(1)), "{id}");
        assert!(f.keywords.is_empty(), "{id}");
        assert_eq!(f.activated_abilities.len(), 2, "{id}");
        let first = activated(f, 0, id);
        let second = activated(f, 1, id);
        assert_costs(first, &[&format!("Mana({first_cost})"), "Tap"], id);
        assert_costs(second, &[&format!("Mana({second_cost})"), "Tap"], id);
        assert_oracle_line(first, 1, id);
        assert_oracle_line(second, 2, id);
    }

    // Ana: {U} flying grant, {B} -2/-0.
    let ana = face(
        "ana_disciple",
        "Ana Disciple",
        "{G}",
        &["Creature", "Human", "Wizard"],
    );
    assert!(
        matches!(
            activated(ana, 0, "ana_disciple").effect.as_slice(),
            [SpellEffectKind::GrantKeywords { subject: EffectSubject::Chosen(_), keywords }]
                if keywords == &[tricerules_cards::Keyword::Flying]
        ),
        "{:?}",
        activated(ana, 0, "ana_disciple").effect
    );
    assert!(
        matches!(
            activated(ana, 1, "ana_disciple").effect.as_slice(),
            [SpellEffectKind::PumpTarget {
                power: -2,
                toughness: 0,
                subject: EffectSubject::Chosen(_),
                ..
            }]
        ),
        "{:?}",
        activated(ana, 1, "ana_disciple").effect
    );

    // Ceta: {R} +2/+0, {G} one mana of any color.
    let ceta = face(
        "ceta_disciple",
        "Ceta Disciple",
        "{U}",
        &["Creature", "Merfolk", "Wizard"],
    );
    assert!(
        matches!(
            activated(ceta, 0, "ceta_disciple").effect.as_slice(),
            [SpellEffectKind::PumpTarget {
                power: 2,
                toughness: 0,
                subject: EffectSubject::Chosen(_),
                ..
            }]
        ),
        "{:?}",
        activated(ceta, 0, "ceta_disciple").effect
    );
    assert_any_color_mana(ceta, 1, "ceta_disciple");

    // Dega: {B} -2/-0, {R} +2/+0.
    let dega = face(
        "dega_disciple",
        "Dega Disciple",
        "{W}",
        &["Creature", "Human", "Wizard"],
    );
    assert!(
        matches!(
            activated(dega, 0, "dega_disciple").effect.as_slice(),
            [SpellEffectKind::PumpTarget {
                power: -2,
                toughness: 0,
                subject: EffectSubject::Chosen(_),
                ..
            }]
        ),
        "{:?}",
        activated(dega, 0, "dega_disciple").effect
    );
    assert!(
        matches!(
            activated(dega, 1, "dega_disciple").effect.as_slice(),
            [SpellEffectKind::PumpTarget {
                power: 2,
                toughness: 0,
                subject: EffectSubject::Chosen(_),
                ..
            }]
        ),
        "{:?}",
        activated(dega, 1, "dega_disciple").effect
    );

    // Necra: {G} one mana of any color, {W} prevent the next 1 damage to any target.
    let necra = face(
        "necra_disciple",
        "Necra Disciple",
        "{B}",
        &["Creature", "Human", "Wizard"],
    );
    assert_any_color_mana(necra, 0, "necra_disciple");
    assert_one_damage_shield(necra, 1, "necra_disciple");

    // Raka: {W} prevent 1, {U} flying grant.
    let raka = face(
        "raka_disciple",
        "Raka Disciple",
        "{R}",
        &["Creature", "Minotaur", "Wizard"],
    );
    assert_one_damage_shield(raka, 0, "raka_disciple");
    assert!(
        matches!(
            activated(raka, 1, "raka_disciple").effect.as_slice(),
            [SpellEffectKind::GrantKeywords { subject: EffectSubject::Chosen(_), keywords }]
                if keywords == &[tricerules_cards::Keyword::Flying]
        ),
        "{:?}",
        activated(raka, 1, "raka_disciple").effect
    );

    // 02: top-selection. Commune with Evil adds the printed life gain; both flashback cards keep
    // their exact printed alternative cost.
    let commune = face(
        "commune_with_evil",
        "Commune with Evil",
        "{2}{B}",
        &["Sorcery"],
    );
    assert!(
        matches!(
            commune.spell_effect.as_slice(),
            [
                SpellEffectKind::LookChooseOneToHandRestToGraveyard { count: 4 },
                SpellEffectKind::GainLife { amount: _, .. }
            ]
        ),
        "{:?}",
        commune.spell_effect
    );

    let sultai = face(
        "sultai_soothsayer",
        "Sultai Soothsayer",
        "{2}{B}{G}{U}",
        &["Creature", "Snake", "Shaman"],
    );
    assert_eq!((sultai.power, sultai.toughness), (Some(2), Some(5)));
    assert_eq!(sultai.triggered_abilities.len(), 1);
    let trigger = &sultai.triggered_abilities[0];
    assert_eq!(trigger.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert_eq!(
        trigger.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert!(matches!(
        trigger.effect.as_slice(),
        [SpellEffectKind::LookChooseOneToHandRestToGraveyard { count: 4 }]
    ));

    for (id, mana, count, flashback) in [
        ("forbidden_alchemy", "{2}{U}", 4u32, "{6}{B}"),
        ("resentful_revelation", "{1}{B}", 3u32, "{6}{B}"),
    ] {
        let f = face(
            id,
            &title(id),
            mana,
            &[if id == "forbidden_alchemy" {
                "Instant"
            } else {
                "Sorcery"
            }],
        );
        assert!(
            matches!(
                f.spell_effect.as_slice(),
                [SpellEffectKind::LookChooseOneToHandRestToGraveyard { count: c }] if *c == count
            ),
            "{id}: {:?}",
            f.spell_effect
        );
        assert_eq!(
            f.flashback_cost.as_ref().map(ToString::to_string),
            Some(flashback.to_string()),
            "{id}"
        );
    }

    // 03: Aura with one attached modifier and an untargeted source-return ability.
    for (id, name, mana, aura_cost, dp, dt, locks) in [
        (
            "agoraphobia",
            "Agoraphobia",
            "{1}{U}",
            "{2}{U}",
            -5i32,
            0i32,
            false,
        ),
        (
            "cage_of_hands",
            "Cage of Hands",
            "{2}{W}",
            "{1}{W}",
            0,
            0,
            true,
        ),
        (
            "viscerid_armor",
            "Viscerid Armor",
            "{1}{U}",
            "{1}{U}",
            1,
            1,
            false,
        ),
    ] {
        let f = face(id, name, mana, &["Enchantment", "Aura"]);
        assert!(
            matches!(
                f.spell_effect.as_slice(),
                [SpellEffectKind::AuraAttach { .. }]
            ),
            "{id}: {:?}",
            f.spell_effect
        );
        assert_eq!(f.static_abilities.len(), 1, "{id}");
        let StaticAbilityDef::AttachedModifier {
            delta_power,
            delta_toughness,
            restriction,
            ..
        } = &f.static_abilities[0].definition
        else {
            panic!("{id}: expected AttachedModifier");
        };
        assert_eq!((*delta_power, *delta_toughness), (dp, dt), "{id}");
        assert_eq!(restriction.cant_attack, locks, "{id}");
        assert_eq!(restriction.cant_block, locks, "{id}");
        assert_eq!(
            f.static_abilities[0].presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{id}"
        );
        assert_eq!(f.activated_abilities.len(), 1, "{id}");
        let ability = activated(f, 0, id);
        assert_oracle_line(ability, 3, id);
        assert_costs(ability, &[&format!("Mana({aura_cost})")], id);
        assert!(
            matches!(
                ability.effect.as_slice(),
                [SpellEffectKind::ReturnToOwnersHand {
                    subject: EffectSubject::Source
                }]
            ),
            "{id}: {:?}",
            ability.effect
        );
    }
}

fn title(id: &str) -> String {
    match id {
        "forbidden_alchemy" => "Forbidden Alchemy".to_string(),
        "resentful_revelation" => "Resentful Revelation".to_string(),
        other => panic!("unexpected title request {other}"),
    }
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

fn assert_one_damage_shield(f: &CardFace, index: usize, id: &str) {
    let ability = activated(f, index, id);
    assert!(
        matches!(
            ability.effect.as_slice(),
            [SpellEffectKind::PreventNextDamage { amount: 1, target }]
                if target.kind == TargetKind::AnyTarget
        ),
        "{id}: {:?}",
        ability.effect
    );
}

#[test]
fn q68_batch_a_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, fingerprint) in [
        (
            "ana_disciple",
            "Ana Disciple",
            "468dc790684b41e7cb9762edde0444acade5db7f04b43bb76d63500d207ca2c6",
        ),
        (
            "ceta_disciple",
            "Ceta Disciple",
            "0af8077b1e917e019a1b7a370817e79fb7366fa373710990151a61ef21a07d21",
        ),
        (
            "dega_disciple",
            "Dega Disciple",
            "d7fb5c46abd88d48ecbac7586200750b94114989494b9292f3cfffd4d02100e4",
        ),
        (
            "necra_disciple",
            "Necra Disciple",
            "f02d0ebea3e7f4958dabc2682a1c2fb2849ccc9fc12820190733059dd84fcc6f",
        ),
        (
            "raka_disciple",
            "Raka Disciple",
            "68f78f7df63494cb685b08535b40e30dc7a297ead7e2d93eda6fa4167d02115f",
        ),
        (
            "commune_with_evil",
            "Commune with Evil",
            "4f4374f24ff6cfbd34b71715d1c563d7df863f65e69742524a70c5039bdb4d3d",
        ),
        (
            "forbidden_alchemy",
            "Forbidden Alchemy",
            "ff5039cb04d31561739e61d4f8b7e2215aa5e064933284ce30be3ccd748f7adc",
        ),
        (
            "resentful_revelation",
            "Resentful Revelation",
            "81aebf77a9a1c91967209967c276f9914182f73983d51e2e7473e8e81fb1f1c4",
        ),
        (
            "sultai_soothsayer",
            "Sultai Soothsayer",
            "04080e75573f5d27d523df629c110f68f275bd4430049b3b3cb784fe81533107",
        ),
        (
            "agoraphobia",
            "Agoraphobia",
            "4d7e193fc5b04932a2c5f75ab865d125d6d5b24871514d8562fff42229c6ad49",
        ),
        (
            "cage_of_hands",
            "Cage of Hands",
            "5c96844e5f51867834e64f45a7ce3e1d280fe8fcde2ace2822d48bb2c1240fde",
        ),
        (
            "viscerid_armor",
            "Viscerid Armor",
            "baf583f2bb0147e16f5cc93ef57a5e17feb89b0ce7b96edfd228d7ba60fcbde9",
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
