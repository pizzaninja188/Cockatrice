//! Registry conformance for the reviewed Trample cohort.
//!
//! Every included card is a normal-layout creature whose only rules text is Trample.
//! Each included card is executed by the shared actual-card scenarios.
use tricerules_cards::{Keyword, Layout};

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    &'static [&'static str],
    Option<(u32, u32)>,
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "ebony_rhino",
        "Ebony Rhino",
        "{7}",
        &["Artifact", "Creature", "Rhino"],
        &[],
        Some((4, 5)),
        "06d375c77047aa371fc0cbdea553e466d534b93acefbe31155df800f7d54381f",
    ),
    (
        "gorn_captain",
        "Gorn Captain",
        "{3}{G}",
        &["Creature", "Gorn", "Soldier"],
        &[],
        Some((4, 3)),
        "f15c1d646b1ca7f6f939056a9bcb36a9a6f6bf2165bb4b9fe3c0b6b74da573cf",
    ),
    (
        "rootbreaker_wurm",
        "Rootbreaker Wurm",
        "{5}{G}{G}",
        &["Creature", "Wurm"],
        &[],
        Some((6, 6)),
        "25612a13b0af23dc726fb197810b666ef5c64cf679cc04ef481891c37c25171f",
    ),
    (
        "war_mammoth",
        "War Mammoth",
        "{3}{G}",
        &["Creature", "Elephant"],
        &[],
        Some((3, 3)),
        "06d375c77047aa371fc0cbdea553e466d534b93acefbe31155df800f7d54381f",
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
fn trample_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, types, supertypes, body, _) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        let f = face(registry, id);
        assert_eq!(f.face_id.as_str(), id.replace('-', "_"), "{id} face id");
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(
            f.types.iter().map(String::as_str).collect::<Vec<_>>(),
            types,
            "{id} types"
        );
        assert_eq!(
            f.supertypes.iter().map(String::as_str).collect::<Vec<_>>(),
            supertypes,
            "{id} supertypes"
        );
        assert_eq!(
            (f.power, f.toughness),
            body.map_or((None, None), |(p, t)| (Some(p), Some(t))),
            "{id} body"
        );
        assert_eq!(f.keywords, [Keyword::Trample], "{id} keywords");
        assert!(f.spell_effect.is_empty(), "{id}");
        assert!(f.targeting.is_none(), "{id}");
        assert!(f.protections.is_empty(), "{id}");
        assert!(f.activated_abilities.is_empty(), "{id}");
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
        assert!(!f.must_attack_if_able, "{id}");
    }
}

#[test]
fn trample_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, _, _, _, _, expected) in CARDS {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(fields[2], id.replace('-', "_"));
        assert_eq!(fields[4], expected, "fingerprint drift for {id}");
    }
}
