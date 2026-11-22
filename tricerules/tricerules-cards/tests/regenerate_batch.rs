//! Registry conformance for the reviewed regenerate creature family.
//!
//! Every included card is a normal-layout creature with one `{cost}: Regenerate this creature.`
//! activated ability. Card names, costs, types and stats are printed data; the shared scenario
//! executes every card. Rimebound Dead is excluded by the snow-mana blocker (issue #516). Each
//! fingerprint is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{AbilityCost, EffectSubject, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, ActivationTiming, Layout};

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    u32,
    u32,
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "ancient_silverback",
        "Ancient Silverback",
        "{4}{G}{G}",
        &["Creature", "Ape"],
        6,
        5,
        "{G}",
    ),
    (
        "clay_statue",
        "Clay Statue",
        "{4}",
        &["Artifact", "Creature", "Golem"],
        3,
        1,
        "{2}",
    ),
    (
        "diabolic_machine",
        "Diabolic Machine",
        "{7}",
        &["Artifact", "Creature", "Construct"],
        4,
        4,
        "{3}",
    ),
    (
        "dutiful_thrull",
        "Dutiful Thrull",
        "{W}",
        &["Creature", "Thrull"],
        1,
        1,
        "{B}",
    ),
    (
        "horned_troll",
        "Horned Troll",
        "{2}{G}",
        &["Creature", "Troll"],
        2,
        2,
        "{G}",
    ),
    (
        "metathran_zombie",
        "Metathran Zombie",
        "{1}{U}",
        &["Creature", "Metathran", "Zombie"],
        1,
        1,
        "{B}",
    ),
    (
        "phyrexian_monitor",
        "Phyrexian Monitor",
        "{3}{B}",
        &["Creature", "Phyrexian", "Skeleton"],
        2,
        2,
        "{B}",
    ),
    (
        "restless_dead",
        "Restless Dead",
        "{1}{B}",
        &["Creature", "Skeleton"],
        1,
        1,
        "{B}",
    ),
    (
        "revered_dead",
        "Revered Dead",
        "{1}{W}",
        &["Creature", "Spirit", "Soldier"],
        1,
        1,
        "{W}",
    ),
    (
        "skeletal_wurm",
        "Skeletal Wurm",
        "{7}{B}",
        &["Creature", "Skeleton", "Wurm"],
        7,
        6,
        "{B}",
    ),
    (
        "unworthy_dead",
        "Unworthy Dead",
        "{1}{B}",
        &["Creature", "Phyrexian", "Skeleton"],
        1,
        1,
        "{B}",
    ),
    (
        "uthden_troll",
        "Uthden Troll",
        "{2}{R}",
        &["Creature", "Troll"],
        2,
        2,
        "{R}",
    ),
    (
        "walking_dead",
        "Walking Dead",
        "{1}{B}",
        &["Creature", "Zombie"],
        1,
        1,
        "{B}",
    ),
];

type Fingerprint = (&'static str, &'static str, &'static str);

const FINGERPRINTS: &[Fingerprint] = &[
    (
        "ancient_silverback",
        "Ancient Silverback",
        "ab2b4b3e412d2f38658867ed9c2229f87df153b72973987d2fb580890b933709",
    ),
    (
        "clay_statue",
        "Clay Statue",
        "d895d494082aceb50939d80b4e5ab1780fa0c7771976d62eed81ee86bfd9f2ab",
    ),
    (
        "diabolic_machine",
        "Diabolic Machine",
        "9f2a6b1f4e64a5f787fd54e24b0e81923961f6524a12c5b7abc569b2bc5d0b75",
    ),
    (
        "dutiful_thrull",
        "Dutiful Thrull",
        "8b04509688b929df87398e2b998a5281a2cced08222e2c99ecabea5900a0b845",
    ),
    (
        "horned_troll",
        "Horned Troll",
        "1d88f7e99fd6eb8894f38ce87c904f2b7466c515b9fdcbe163af094121765696",
    ),
    (
        "metathran_zombie",
        "Metathran Zombie",
        "8b04509688b929df87398e2b998a5281a2cced08222e2c99ecabea5900a0b845",
    ),
    (
        "phyrexian_monitor",
        "Phyrexian Monitor",
        "8b04509688b929df87398e2b998a5281a2cced08222e2c99ecabea5900a0b845",
    ),
    (
        "restless_dead",
        "Restless Dead",
        "8b04509688b929df87398e2b998a5281a2cced08222e2c99ecabea5900a0b845",
    ),
    (
        "revered_dead",
        "Revered Dead",
        "c8a7586721573532937b563297b482800c11ea58384bd9469d07f9d14222f7fc",
    ),
    (
        "skeletal_wurm",
        "Skeletal Wurm",
        "8b04509688b929df87398e2b998a5281a2cced08222e2c99ecabea5900a0b845",
    ),
    (
        "unworthy_dead",
        "Unworthy Dead",
        "8b04509688b929df87398e2b998a5281a2cced08222e2c99ecabea5900a0b845",
    ),
    (
        "uthden_troll",
        "Uthden Troll",
        "b8f8c15feaaa6579cf8e5cd1f63b7d744b0e0f5a291d2bab4588b1108cb98666",
    ),
    (
        "walking_dead",
        "Walking Dead",
        "8b04509688b929df87398e2b998a5281a2cced08222e2c99ecabea5900a0b845",
    ),
];

fn face<'a>(
    registry: &'a tricerules_cards::CardRegistry,
    id: &str,
) -> &'a tricerules_cards::CardFace {
    registry
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
        .primary_face()
}

#[test]
fn regenerate_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, types, power, toughness, regenerate) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        let f = face(registry, id);
        assert_eq!(f.face_id.as_str(), id, "{id} face id");
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(f.types, types, "{id} types");
        assert_eq!(f.power, Some(power), "{id} power");
        assert_eq!(f.toughness, Some(toughness), "{id} toughness");

        let [ability] = f.activated_abilities.as_slice() else {
            panic!("{id}: one activated ability");
        };
        assert_eq!(ability.ability_id.as_str(), "activated_01", "{id}");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id} presentation"
        );
        assert_eq!(ability.timing, ActivationTiming::Normal, "{id} timing");
        assert!(
            matches!(
                ability.costs.as_slice(),
                [AbilityCost::Mana(cost)] if cost.to_string() == regenerate
            ),
            "{id}: {:?}",
            ability.costs
        );
        assert!(
            matches!(
                ability.effect.as_slice(),
                [SpellEffectKind::Regenerate {
                    subject: EffectSubject::Source,
                }]
            ),
            "{id}: {:?}",
            ability.effect
        );
        assert!(
            ability.targeting.is_none(),
            "{id}: regeneration has no targets"
        );
    }
}

#[test]
fn regenerate_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, fingerprint) in FINGERPRINTS {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(fields[2], id);
        assert_eq!(fields[4], fingerprint, "fingerprint drift for {id}");
    }
}
