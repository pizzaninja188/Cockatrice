//! Registry conformance for the reviewed "deals N damage to any target" direct-RON family.
//!
//! The nineteen cards share one Oracle shape and differ only in printed name, mana cost, type
//! line, damage amount, and (for Gut Shot) a Phyrexian mana symbol. Gut Shot's reminder line is
//! presentation mapping only. Tarfire is Kindred, a card type under CR 308.2, with the shared
//! Goblin creature subtype. Each fingerprint is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{Amount, SpellEffectKind, TargetKind};
use tricerules_cards::{CardRegistry, Layout};

const CARDS: &[(&str, &str, &str, &[&str], u32)] = &[
    ("bee_sting", "Bee Sting", "{3}{G}", &["Sorcery"], 2),
    ("cinder_storm", "Cinder Storm", "{6}{R}", &["Sorcery"], 7),
    (
        "cleansing_screech",
        "Cleansing Screech",
        "{4}{R}",
        &["Sorcery"],
        4,
    ),
    (
        "explosive_impact",
        "Explosive Impact",
        "{5}{R}",
        &["Instant"],
        5,
    ),
    ("fire_ambush", "Fire Ambush", "{1}{R}", &["Sorcery"], 3),
    ("gintak_charge", "Gin'tak Charge", "{R}", &["Sorcery"], 2),
    ("gut_shot", "Gut Shot", "{R/P}", &["Instant"], 1),
    ("hornet_sting", "Hornet Sting", "{G}", &["Instant"], 1),
    (
        "lightning_blast",
        "Lightning Blast",
        "{3}{R}",
        &["Instant"],
        4,
    ),
    ("open_fire", "Open Fire", "{2}{R}", &["Instant"], 3),
    (
        "precision_bolt",
        "Precision Bolt",
        "{2}{R}",
        &["Sorcery"],
        3,
    ),
    ("scorching_spear", "Scorching Spear", "{R}", &["Sorcery"], 1),
    ("searing_spear", "Searing Spear", "{1}{R}", &["Instant"], 3),
    ("searing_wind", "Searing Wind", "{8}{R}", &["Instant"], 10),
    (
        "tarfire",
        "Tarfire",
        "{R}",
        &["Kindred", "Instant", "Goblin"],
        2,
    ),
    (
        "unfriendly_fire",
        "Unfriendly Fire",
        "{4}{R}",
        &["Instant"],
        4,
    ),
    (
        "unyaro_bee_sting",
        "Unyaro Bee Sting",
        "{3}{G}",
        &["Sorcery"],
        2,
    ),
    (
        "volcanic_hammer",
        "Volcanic Hammer",
        "{1}{R}",
        &["Sorcery"],
        3,
    ),
    ("zukos_offense", "Zuko's Offense", "{R}", &["Sorcery"], 2),
];

fn face<'a>(registry: &'a CardRegistry, id: &str) -> &'a tricerules_cards::CardFace {
    registry
        .get(id)
        .unwrap_or_else(|| panic!("missing {id}"))
        .primary_face()
}

#[test]
fn damage_any_target_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, types, amount) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        let f = definition.primary_face();
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(f.types, types, "{id} types");
        let [effect] = f.spell_effect.as_slice() else {
            panic!("{id}: one damage effect");
        };
        assert!(
            matches!(
                effect,
                SpellEffectKind::DamageTarget { amount: a, target }
                    if *a == Amount::Fixed(amount) && target.kind == TargetKind::AnyTarget
            ),
            "{id}: {effect:?}"
        );
    }

    // Tarfire's kindred card type and Goblin subtype are its complete printed type line.
    assert_eq!(
        face(registry, "tarfire").types,
        ["Kindred", "Instant", "Goblin"]
    );
}

#[test]
fn damage_any_target_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, fingerprint) in [
        (
            "bee_sting",
            "Bee Sting",
            "4ab3029e7e027eebb7fe17afadbeee7ef808fe4c0042d7d84e26c5706f4f8248",
        ),
        (
            "cinder_storm",
            "Cinder Storm",
            "f9f8f654a0014d695103ebd3fd82f45c9a8d55ffc38c20305a7c5a97a9f61995",
        ),
        (
            "cleansing_screech",
            "Cleansing Screech",
            "cc8f93a1674d5c04a20acee623ca4da1cf5da2ada3446d2a6684433829d0727b",
        ),
        (
            "explosive_impact",
            "Explosive Impact",
            "927f8d81acfaff79f814bb4bfaa44b5f4be6550f17b37c04c019e6f0a080fbe4",
        ),
        (
            "fire_ambush",
            "Fire Ambush",
            "f924c6f1fd7a2c852bf5c8a6a64218e42341a902f885292ab29c3c76df9efebe",
        ),
        (
            "gintak_charge",
            "Gin'tak Charge",
            "ab55dac428d321afddf22d99aa78d0e7271b3f554bb1d5b7f623db2b9e05169d",
        ),
        (
            "gut_shot",
            "Gut Shot",
            "a11327cb59bf5667c5a986125eb40de789870d3fdd9be4b1bc802eea0c7fd752",
        ),
        (
            "hornet_sting",
            "Hornet Sting",
            "41ed234e5970998d383a8c60e14a5270eb2ef6b90b77ea13b2105a5bd4311c68",
        ),
        (
            "lightning_blast",
            "Lightning Blast",
            "e7e6a34d6e28a4216cf3ded077ba263fb5eba29722156f58243b3349027dd533",
        ),
        (
            "open_fire",
            "Open Fire",
            "57b99b191ad85320148660e49c14ce7710061970012d6140ddbc2c6a8ebb84bf",
        ),
        (
            "precision_bolt",
            "Precision Bolt",
            "ddc27b59da80fa27483caa97ccc8f3660f3e49ed378eaefdddcd2a75cabda58d",
        ),
        (
            "scorching_spear",
            "Scorching Spear",
            "cfa340431eba248079255bf1aa2b9bd785bff007e05f8b016fdf85cf257be200",
        ),
        (
            "searing_spear",
            "Searing Spear",
            "3e1f135ef845da821df564a37f6f1079b0d1b3c49b8000da5229a8b9fb75d072",
        ),
        (
            "searing_wind",
            "Searing Wind",
            "b92eaf530b07676caa06ad094372c931cdd84efc5d9c386489ee6dce29f04adf",
        ),
        (
            "tarfire",
            "Tarfire",
            "0f92cdcb901681b282f55d72e472419d9885545c270e4adbd5623605130d343b",
        ),
        (
            "unfriendly_fire",
            "Unfriendly Fire",
            "89449bedd144276b30dd60d317ca22d50d1e4cb5c11c115a65b5bbd02a0584a4",
        ),
        (
            "unyaro_bee_sting",
            "Unyaro Bee Sting",
            "a7f53c7bac2524fdde15bca9cf143734822ae09a4f877f0d5a5d0ff4e0ab3dca",
        ),
        (
            "volcanic_hammer",
            "Volcanic Hammer",
            "8ed741e909ba4487bd43308cbe3c689efe08b1b8b1ffbb87ab6efd0938004e87",
        ),
        (
            "zukos_offense",
            "Zuko's Offense",
            "c5a824799ac741156dacada9fd1720171c1e95bf3f2e539500117e26fe73b7cd",
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
        assert_eq!(fields[4], fingerprint, "fingerprint drift for {id}");
    }
}
