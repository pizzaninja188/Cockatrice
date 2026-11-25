//! Registry conformance for the queue-v75 03-surveil-cantrips cohort.
//!
//! One surveil instruction before the printed draw, or the mass shrink followed by surveil.
//! Fingerprints are checked after the presentation metadata refresh.

use tricerules_cards::primitives::{Amount, LibraryPartitionKind, SpellEffectKind};
use tricerules_cards::{registry, CardFace, Layout};

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

fn assert_surveil(effect: &SpellEffectKind, count: u32, id: &str) {
    assert!(
        matches!(
            effect,
            SpellEffectKind::LibraryPartition {
                count: found,
                top_min: 0,
                top_max: None,
                kind: LibraryPartitionKind::Surveil,
            } if *found == count
        ),
        "{id}: surveil {count}"
    );
}

#[test]
fn q75_surveil_cantrips_map_definitions() {
    let consider = face("consider", "Consider", "{U}", &["Instant"]);
    assert_eq!(consider.spell_effect.len(), 2);
    assert_surveil(&consider.spell_effect[0], 1, "consider");
    assert!(
        matches!(
            &consider.spell_effect[1],
            SpellEffectKind::Draw {
                count: Amount::Fixed(1),
                ..
            }
        ),
        "Consider draws one after surveilling"
    );

    let curate = face("curate", "Curate", "{1}{U}", &["Instant"]);
    assert_eq!(curate.spell_effect.len(), 2);
    assert_surveil(&curate.spell_effect[0], 2, "curate");
    assert!(
        matches!(
            &curate.spell_effect[1],
            SpellEffectKind::Draw {
                count: Amount::Fixed(1),
                ..
            }
        ),
        "Curate draws one after surveilling"
    );

    let vapors = face("mephitic_vapors", "Mephitic Vapors", "{2}{B}", &["Sorcery"]);
    assert_eq!(vapors.spell_effect.len(), 2);
    assert!(
        matches!(
            &vapors.spell_effect[0],
            SpellEffectKind::PumpAll {
                filter,
                power: -1,
                toughness: -1,
            } if *filter == Default::default()
        ),
        "Mephitic Vapors gives every creature -1/-1"
    );
    assert_surveil(&vapors.spell_effect[1], 2, "mephitic_vapors");

    for f in [consider, curate, vapors] {
        assert!(f.spell_keywords.is_empty(), "no spell keyword");
        assert!(f.flashback_cost.is_none(), "no flashback");
        assert!(f.static_abilities.is_empty(), "no static ability");
        assert!(f.activated_abilities.is_empty(), "no activated ability");
        assert!(f.targeting.is_none(), "{} is untargeted", f.name);
    }
}

#[test]
fn q75_surveil_cantrips_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for (id, name, face_id, fingerprint) in [
        (
            "consider",
            "Consider",
            "consider",
            "09d6b1512d351d402a63efdf1267efc15e1fa22fc97ab2d2db7a61f4e639e0d9",
        ),
        (
            "curate",
            "Curate",
            "curate",
            "d07d8efbcfabb39ac1d2eaeed5aaa4120c6960bdc72368a903874dece6d9e503",
        ),
        (
            "mephitic_vapors",
            "Mephitic Vapors",
            "mephitic_vapors",
            "3e6306f39ff7be7d41166beeb60e45d8982271d4554585421bab3f8b59b04184",
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
