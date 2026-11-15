//! Registry conformance for the reviewed Convoke creature family.
//!
//! Every included card is a normal-layout creature with Convoke and an optional
//! printed evergreen keyword. Each included card is executed by the shared
//! actual-card scenarios.
use tricerules_cards::{Keyword, Layout};

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    &'static [&'static str],
    Option<(u32, u32)>,
    &'static [Keyword],
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "halo_hopper",
        "Halo Hopper",
        "{3}",
        &["Artifact", "Creature", "Frog"],
        &[],
        Some((3, 2)),
        &[Keyword::Convoke],
        "1709c8b31d9ba8e36bfe4067301346cab1c718dc738bde3f13b1c3748a4498b5",
    ),
    (
        "ledev_guardian",
        "Ledev Guardian",
        "{3}{W}",
        &["Creature", "Human", "Knight"],
        &[],
        Some((2, 4)),
        &[Keyword::Convoke],
        "1709c8b31d9ba8e36bfe4067301346cab1c718dc738bde3f13b1c3748a4498b5",
    ),
    (
        "will-forged_golem",
        "Will-Forged Golem",
        "{6}",
        &["Artifact", "Creature", "Golem"],
        &[],
        Some((4, 4)),
        &[Keyword::Convoke],
        "1709c8b31d9ba8e36bfe4067301346cab1c718dc738bde3f13b1c3748a4498b5",
    ),
    (
        "conclave_equenaut",
        "Conclave Equenaut",
        "{4}{W}{W}",
        &["Creature", "Human", "Soldier"],
        &[],
        Some((3, 3)),
        &[Keyword::Convoke, Keyword::Flying],
        "863831e671718b93f96ecc12ca3da94d29540760228986f38cd801bec36c259e",
    ),
    (
        "flight_of_equenauts",
        "Flight of Equenauts",
        "{7}{W}",
        &["Creature", "Human", "Knight"],
        &[],
        Some((4, 5)),
        &[Keyword::Convoke, Keyword::Flying],
        "863831e671718b93f96ecc12ca3da94d29540760228986f38cd801bec36c259e",
    ),
    (
        "thunderhead_squadron",
        "Thunderhead Squadron",
        "{5}{U}",
        &["Creature", "Human", "Knight"],
        &[],
        Some((3, 4)),
        &[Keyword::Convoke, Keyword::Flying],
        "863831e671718b93f96ecc12ca3da94d29540760228986f38cd801bec36c259e",
    ),
    (
        "autochthon_wurm",
        "Autochthon Wurm",
        "{10}{G}{G}{G}{W}{W}",
        &["Creature", "Wurm"],
        &[],
        Some((9, 14)),
        &[Keyword::Convoke, Keyword::Trample],
        "e8e9a355af3e41426bb571e1df06f33c5620a69af575c2f8b6260fb2042a2798",
    ),
    (
        "siege_wurm",
        "Siege Wurm",
        "{5}{G}{G}",
        &["Creature", "Wurm"],
        &[],
        Some((5, 5)),
        &[Keyword::Convoke, Keyword::Trample],
        "e8e9a355af3e41426bb571e1df06f33c5620a69af575c2f8b6260fb2042a2798",
    ),
    (
        "guardian_of_vitu-ghazi",
        "Guardian of Vitu-Ghazi",
        "{6}{G}{W}",
        &["Creature", "Elemental"],
        &[],
        Some((4, 7)),
        &[Keyword::Convoke, Keyword::Vigilance],
        "78e8885cd97cd7131a105846ae05543c654727c6db23a20f92b423e5a7c2a879",
    ),
    (
        "rosemane_centaur",
        "Rosemane Centaur",
        "{3}{G}{W}",
        &["Creature", "Centaur", "Soldier"],
        &[],
        Some((4, 4)),
        &[Keyword::Convoke, Keyword::Vigilance],
        "78e8885cd97cd7131a105846ae05543c654727c6db23a20f92b423e5a7c2a879",
    ),
    (
        "arboretum_elemental",
        "Arboretum Elemental",
        "{7}{G}{G}",
        &["Creature", "Elemental"],
        &[],
        Some((7, 5)),
        &[Keyword::Convoke, Keyword::Hexproof],
        "38e8566f417adb8d4e2815f536e2671a62ec4508e32251e34e144b52f67c631e",
    ),
    (
        "impervious_greatwurm",
        "Impervious Greatwurm",
        "{7}{G}{G}{G}",
        &["Creature", "Wurm"],
        &[],
        Some((16, 16)),
        &[Keyword::Convoke, Keyword::Indestructible],
        "87466c7c78680a944f3c3e484fd061e320c69cd3323a47388910a4a158b37d95",
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
fn convoke_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, types, supertypes, body, keywords, _) in CARDS {
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
        assert_eq!(f.keywords, keywords, "{id} keywords");
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
fn convoke_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, _, _, _, _, _, expected) in CARDS {
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
