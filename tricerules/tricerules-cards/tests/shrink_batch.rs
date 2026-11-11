//! Registry conformance for the reviewed shrink-target spell family.
//!
//! Every card is a normal-layout Instant or Sorcery with one `PumpTarget` using negative printed
//! power/toughness. Dismember adds the Phyrexian cost and a two-line presentation. Card names,
//! costs, types and pump values are printed data; the shared scenario executes every card. Each
//! fingerprint is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::SpellEffectKind;
use tricerules_cards::Layout;

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    i32,
    i32,
);

const CARDS: &[Card] = &[
    (
        "demons_grasp",
        "Demon's Grasp",
        "{4}{B}",
        &["Sorcery"],
        -5,
        -5,
    ),
    ("disfigure", "Disfigure", "{B}", &["Instant"], -2, -2),
    (
        "dismember",
        "Dismember",
        "{1}{B/P}{B/P}",
        &["Instant"],
        -5,
        -5,
    ),
    ("disorient", "Disorient", "{3}{U}", &["Instant"], -7, 0),
    (
        "eyes_of_the_beholder",
        "Eyes of the Beholder",
        "{4}{B}{B}",
        &["Instant"],
        -11,
        -11,
    ),
    ("fatal_fumes", "Fatal Fumes", "{3}{B}", &["Instant"], -4, -2),
    ("flatten", "Flatten", "{3}{B}", &["Instant"], -4, -4),
    (
        "grasp_of_darkness",
        "Grasp of Darkness",
        "{B}{B}",
        &["Instant"],
        -4,
        -4,
    ),
    ("hydrosurge", "Hydrosurge", "{U}", &["Instant"], -5, 0),
    (
        "lash_of_the_whip",
        "Lash of the Whip",
        "{4}{B}",
        &["Instant"],
        -4,
        -4,
    ),
    (
        "pull_under",
        "Pull Under",
        "{5}{B}",
        &["Instant", "Arcane"],
        -5,
        -5,
    ),
    ("shrink", "Shrink", "{G}", &["Instant"], -5, 0),
    (
        "strangling_spores",
        "Strangling Spores",
        "{3}{B}",
        &["Instant"],
        -3,
        -3,
    ),
    ("tar_snare", "Tar Snare", "{2}{B}", &["Instant"], -3, -2),
    ("throttle", "Throttle", "{4}{B}", &["Instant"], -4, -4),
    ("wring_flesh", "Wring Flesh", "{B}", &["Instant"], -3, -1),
];

type Fingerprint = (&'static str, &'static str, &'static str);

const FINGERPRINTS: &[Fingerprint] = &[
    (
        "demons_grasp",
        "Demon's Grasp",
        "617372107c755f0b210aa44262a00ab1d78c6b27de5c180951c1de3723d7f92b",
    ),
    (
        "disfigure",
        "Disfigure",
        "163493e09fa755597a2009884cbbc58212823ef479ff493c8b2f6c6daf6d4544",
    ),
    (
        "dismember",
        "Dismember",
        "43cabab42dd2e3f867b2c58bea3353fe1b138f2f9b854536f0fe0459ec4e3b7e",
    ),
    (
        "disorient",
        "Disorient",
        "9d4211e0e42e2ab7f7d51f1ec88e7561cdbc6cb4d89c9cd72c110fb4223db62c",
    ),
    (
        "eyes_of_the_beholder",
        "Eyes of the Beholder",
        "d0ebf489de63da72429300bad481402bca781498dfe53fbc25bae433595760a8",
    ),
    (
        "fatal_fumes",
        "Fatal Fumes",
        "59ea838d2545f47b7bb418b185b782bceb953976ce522c239cbdeb8ac5de73cf",
    ),
    (
        "flatten",
        "Flatten",
        "be69225ab88ad8cfba49baae01c9b5d81b3d8a14549329dff337313624ee9855",
    ),
    (
        "grasp_of_darkness",
        "Grasp of Darkness",
        "be69225ab88ad8cfba49baae01c9b5d81b3d8a14549329dff337313624ee9855",
    ),
    (
        "hydrosurge",
        "Hydrosurge",
        "9da76ddcc07b2516e2772a87925bb2957bd6c7cd50307150d0b2cfb032590d16",
    ),
    (
        "lash_of_the_whip",
        "Lash of the Whip",
        "be69225ab88ad8cfba49baae01c9b5d81b3d8a14549329dff337313624ee9855",
    ),
    (
        "pull_under",
        "Pull Under",
        "617372107c755f0b210aa44262a00ab1d78c6b27de5c180951c1de3723d7f92b",
    ),
    (
        "shrink",
        "Shrink",
        "9da76ddcc07b2516e2772a87925bb2957bd6c7cd50307150d0b2cfb032590d16",
    ),
    (
        "strangling_spores",
        "Strangling Spores",
        "e8bc8350ab1500521fd7daf2f9f217f10080e2ffaa2b0ee523fe94b5b197fb94",
    ),
    (
        "tar_snare",
        "Tar Snare",
        "db419884506fee019d65dc5d5f2b79e67d04b3892906b78af8af4db2c4630e7a",
    ),
    (
        "throttle",
        "Throttle",
        "be69225ab88ad8cfba49baae01c9b5d81b3d8a14549329dff337313624ee9855",
    ),
    (
        "wring_flesh",
        "Wring Flesh",
        "be85b9ff28b5df27d4512a6f4cf4dae340ab00af6f4cff13df902834fa2f73ca",
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
fn shrink_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, types, power, toughness) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        let f = face(registry, id);
        assert_eq!(f.face_id.as_str(), id, "{id} face id");
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(f.types, types, "{id} types");
        assert_eq!(f.power, None, "{id}: spells have no power");
        assert_eq!(f.toughness, None, "{id}: spells have no toughness");

        let [effect] = f.spell_effect.as_slice() else {
            panic!("{id}: one spell effect");
        };
        assert!(
            matches!(
                effect,
                SpellEffectKind::PumpTarget {
                    power: p,
                    toughness: t,
                    scale: None,
                    ..
                } if *p == power && *t == toughness
            ),
            "{id}: {:?}",
            f.spell_effect
        );
    }
}

#[test]
fn shrink_batch_fingerprints_match_the_pinned_oracle_text() {
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
