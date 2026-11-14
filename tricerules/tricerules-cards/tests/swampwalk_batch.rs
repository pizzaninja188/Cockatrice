//! Registry conformance for the reviewed Swampwalk creature family.
//!
//! Every card is a normal-layout creature whose only rules text is Swampwalk.
//! Each fingerprint is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::Evasion;
use tricerules_cards::Layout;

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
        "anaconda",
        "Anaconda",
        "{3}{G}",
        &["Creature", "Snake"],
        3,
        3,
        "0cb8985ac7da064a0e285c23eebb532df82b4d74b55dae5903f3726345e47588",
    ),
    (
        "anurid_murkdiver",
        "Anurid Murkdiver",
        "{4}{B}{B}",
        &["Creature", "Zombie", "Frog", "Beast"],
        4,
        3,
        "0cb8985ac7da064a0e285c23eebb532df82b4d74b55dae5903f3726345e47588",
    ),
    (
        "bog_raiders",
        "Bog Raiders",
        "{2}{B}",
        &["Creature", "Zombie"],
        2,
        2,
        "0cb8985ac7da064a0e285c23eebb532df82b4d74b55dae5903f3726345e47588",
    ),
    (
        "bog_smugglers",
        "Bog Smugglers",
        "{1}{B}{B}",
        &["Creature", "Human", "Mercenary"],
        2,
        2,
        "0cb8985ac7da064a0e285c23eebb532df82b4d74b55dae5903f3726345e47588",
    ),
    (
        "bog_tatters",
        "Bog Tatters",
        "{4}{B}",
        &["Creature", "Wraith"],
        4,
        2,
        "0cb8985ac7da064a0e285c23eebb532df82b4d74b55dae5903f3726345e47588",
    ),
    (
        "bog_wraith",
        "Bog Wraith",
        "{3}{B}",
        &["Creature", "Wraith"],
        3,
        3,
        "0cb8985ac7da064a0e285c23eebb532df82b4d74b55dae5903f3726345e47588",
    ),
    (
        "farbog_explorer",
        "Farbog Explorer",
        "{2}{W}",
        &["Creature", "Human", "Scout"],
        2,
        3,
        "0cb8985ac7da064a0e285c23eebb532df82b4d74b55dae5903f3726345e47588",
    ),
    (
        "lost_soul",
        "Lost Soul",
        "{1}{B}{B}",
        &["Creature", "Spirit", "Minion"],
        2,
        1,
        "0cb8985ac7da064a0e285c23eebb532df82b4d74b55dae5903f3726345e47588",
    ),
    (
        "marsh_boa",
        "Marsh Boa",
        "{G}",
        &["Creature", "Snake"],
        1,
        1,
        "0cb8985ac7da064a0e285c23eebb532df82b4d74b55dae5903f3726345e47588",
    ),
    (
        "marsh_goblins",
        "Marsh Goblins",
        "{B}{R}",
        &["Creature", "Goblin"],
        1,
        1,
        "0cb8985ac7da064a0e285c23eebb532df82b4d74b55dae5903f3726345e47588",
    ),
    (
        "marsh_threader",
        "Marsh Threader",
        "{1}{W}",
        &["Creature", "Kor", "Scout"],
        2,
        1,
        "0cb8985ac7da064a0e285c23eebb532df82b4d74b55dae5903f3726345e47588",
    ),
    (
        "moor_fiend",
        "Moor Fiend",
        "{3}{B}",
        &["Creature", "Horror"],
        3,
        3,
        "0cb8985ac7da064a0e285c23eebb532df82b4d74b55dae5903f3726345e47588",
    ),
    (
        "plague_beetle",
        "Plague Beetle",
        "{B}",
        &["Creature", "Insect"],
        1,
        1,
        "0cb8985ac7da064a0e285c23eebb532df82b4d74b55dae5903f3726345e47588",
    ),
    (
        "pygmy_allosaurus",
        "Pygmy Allosaurus",
        "{2}{G}",
        &["Creature", "Dinosaur"],
        2,
        2,
        "0cb8985ac7da064a0e285c23eebb532df82b4d74b55dae5903f3726345e47588",
    ),
    (
        "raiding_nightstalker",
        "Raiding Nightstalker",
        "{2}{B}",
        &["Creature", "Nightstalker"],
        2,
        2,
        "0cb8985ac7da064a0e285c23eebb532df82b4d74b55dae5903f3726345e47588",
    ),
    (
        "warthog",
        "Warthog",
        "{1}{G}{G}",
        &["Creature", "Boar"],
        3,
        2,
        "0cb8985ac7da064a0e285c23eebb532df82b4d74b55dae5903f3726345e47588",
    ),
    (
        "wild_ox",
        "Wild Ox",
        "{3}{G}",
        &["Creature", "Ox"],
        3,
        3,
        "0cb8985ac7da064a0e285c23eebb532df82b4d74b55dae5903f3726345e47588",
    ),
    (
        "zodiac_ox",
        "Zodiac Ox",
        "{3}{G}",
        &["Creature", "Ox"],
        3,
        3,
        "0cb8985ac7da064a0e285c23eebb532df82b4d74b55dae5903f3726345e47588",
    ),
    (
        "zodiac_pig",
        "Zodiac Pig",
        "{3}{B}",
        &["Creature", "Boar"],
        3,
        3,
        "0cb8985ac7da064a0e285c23eebb532df82b4d74b55dae5903f3726345e47588",
    ),
    (
        "zodiac_rat",
        "Zodiac Rat",
        "{B}",
        &["Creature", "Rat"],
        1,
        1,
        "0cb8985ac7da064a0e285c23eebb532df82b4d74b55dae5903f3726345e47588",
    ),
    (
        "zodiac_snake",
        "Zodiac Snake",
        "{2}{B}",
        &["Creature", "Snake"],
        2,
        2,
        "0cb8985ac7da064a0e285c23eebb532df82b4d74b55dae5903f3726345e47588",
    ),
];

#[test]
fn swampwalk_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, types, power, toughness, _) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        let f = face(registry, id);
        assert_eq!(f.face_id.as_str(), id.replace('-', "_"), "{id} face id");
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(f.types, types, "{id} types");
        assert_eq!(
            (f.power, f.toughness),
            (Some(power), Some(toughness)),
            "{id} body"
        );
        assert_eq!(
            f.evasions,
            [Evasion::Landwalk {
                land_subtype: "Swamp".to_string()
            }],
            "{id} Swampwalk"
        );
        assert!(f.keywords.is_empty(), "{id}");
        assert!(f.protections.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
        assert!(f.activated_abilities.is_empty(), "{id}");
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.spell_effect.is_empty(), "{id}");
    }
}

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
fn swampwalk_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, .., expected) in CARDS {
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
