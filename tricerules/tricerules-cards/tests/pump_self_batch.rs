//! Registry conformance for the reviewed flying self-pump creature family.
//!
//! Every included card is a normal-layout creature with flying and one
//! `{cost}: This creature gets +N/+M until end of turn.` activated ability. Card names, costs,
//! types and stats are printed data; the shared scenario executes every card. Chilling Shade is
//! excluded by the snow-mana blocker (issue #516). Each fingerprint is SHA-256 of the exact
//! pinned normalized Oracle text.

use tricerules_cards::primitives::{AbilityCost, EffectSubject, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, Keyword, Layout, ManaCost};

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    u32,
    u32,
    &'static str,
    i32,
    i32,
    u16,
);

const CARDS: &[Card] = &[
    (
        "dragon_hatchling",
        "Dragon Hatchling",
        "{1}{R}",
        &["Creature", "Dragon"],
        0,
        1,
        "{R}",
        1,
        0,
        2,
    ),
    (
        "aven_flock",
        "Aven Flock",
        "{4}{W}",
        &["Creature", "Bird", "Soldier"],
        2,
        3,
        "{W}",
        0,
        1,
        2,
    ),
    (
        "balshan_collaborator",
        "Balshan Collaborator",
        "{3}{U}",
        &["Creature", "Bird", "Soldier"],
        2,
        2,
        "{B}",
        1,
        1,
        2,
    ),
    (
        "blistering_dieflyn",
        "Blistering Dieflyn",
        "{3}{R}",
        &["Creature", "Imp"],
        0,
        1,
        "{B/R}",
        1,
        0,
        2,
    ),
    (
        "drifting_shade",
        "Drifting Shade",
        "{3}{B}",
        &["Creature", "Shade"],
        1,
        1,
        "{B}",
        1,
        1,
        2,
    ),
    (
        "dungeon_shade",
        "Dungeon Shade",
        "{3}{B}",
        &["Creature", "Shade", "Spirit"],
        1,
        1,
        "{B}",
        1,
        1,
        2,
    ),
    (
        "firefly",
        "Firefly",
        "{3}{R}",
        &["Creature", "Insect"],
        1,
        1,
        "{R}",
        1,
        0,
        2,
    ),
    (
        "furnace_whelp",
        "Furnace Whelp",
        "{2}{R}{R}",
        &["Creature", "Dragon"],
        2,
        2,
        "{R}",
        1,
        0,
        2,
    ),
    (
        "granite_gargoyle",
        "Granite Gargoyle",
        "{2}{R}",
        &["Creature", "Gargoyle"],
        2,
        2,
        "{R}",
        0,
        1,
        2,
    ),
    (
        "hellkite_punisher",
        "Hellkite Punisher",
        "{5}{R}{R}",
        &["Creature", "Dragon"],
        6,
        6,
        "{R}",
        1,
        0,
        2,
    ),
    (
        "killer_bees",
        "Killer Bees",
        "{1}{G}{G}",
        &["Creature", "Insect"],
        0,
        1,
        "{G}",
        1,
        1,
        2,
    ),
    (
        "marble_gargoyle",
        "Marble Gargoyle",
        "{2}{W}",
        &["Artifact", "Creature", "Gargoyle"],
        2,
        2,
        "{W}",
        0,
        1,
        2,
    ),
    (
        "moonwing_moth",
        "Moonwing Moth",
        "{1}{W}{W}",
        &["Creature", "Insect"],
        2,
        1,
        "{W}",
        0,
        1,
        2,
    ),
    (
        "tower_drake",
        "Tower Drake",
        "{2}{U}",
        &["Creature", "Drake"],
        2,
        1,
        "{W}",
        0,
        1,
        2,
    ),
    (
        "moltensteel_dragon",
        "Moltensteel Dragon",
        "{4}{R/P}{R/P}",
        &["Artifact", "Creature", "Phyrexian", "Dragon"],
        4,
        4,
        "{R/P}",
        1,
        0,
        3,
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
fn pump_self_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, types, power, toughness, pump_cost, pump_power, pump_toughness, line) in
        CARDS
    {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        let f = face(registry, id);
        assert_eq!(f.face_id.as_str(), id, "{id} face id");
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(f.types, types, "{id} types");
        assert_eq!(f.power, Some(power), "{id} power");
        assert_eq!(f.toughness, Some(toughness), "{id} toughness");
        assert_eq!(f.keywords, [Keyword::Flying], "{id} flying");
        assert!(f.spell_effect.is_empty(), "{id} has no spell effect");
        assert!(
            f.triggered_abilities.is_empty(),
            "{id} has no triggered ability"
        );

        let [ability] = f.activated_abilities.as_slice() else {
            panic!("{id}: exactly one activated ability");
        };
        assert_eq!(ability.ability_id.as_str(), "activated_01", "{id}");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![line]),
            "{id} presentation"
        );
        assert_eq!(
            ability.costs,
            vec![AbilityCost::Mana(
                ManaCost::parse(pump_cost).expect("printed pump cost")
            )],
            "{id} pump cost"
        );
        assert!(
            matches!(
                ability.effect.as_slice(),
                [SpellEffectKind::PumpTarget {
                    power,
                    toughness,
                    scale: None,
                    subject: EffectSubject::Source,
                }] if *power == pump_power && *toughness == pump_toughness
            ),
            "{id}: {:?}",
            ability.effect
        );
        assert!(ability.targeting.is_none(), "{id}: no target");
    }
}

type Fingerprint = (&'static str, &'static str, &'static str);

const FINGERPRINTS: &[Fingerprint] = &[
    (
        "aven_flock",
        "Aven Flock",
        "d5e4a28e6047573c586597be62c1db36768f4f5f3f26bf3c79ae9b742fc81aea",
    ),
    (
        "balshan_collaborator",
        "Balshan Collaborator",
        "4be506057006cc75b0181ff08183fba721bc09bcb1b28c3190c8eb5d72e2badb",
    ),
    (
        "blistering_dieflyn",
        "Blistering Dieflyn",
        "8a71c1cc274849c4c6cda7d3ff316e258f280535d80dc9ef2fb3d99105b77c6a",
    ),
    (
        "dragon_hatchling",
        "Dragon Hatchling",
        "e74575fc5da3990b1a3ad046658d95976f710451bdeb79ad7470464394f5ce04",
    ),
    (
        "drifting_shade",
        "Drifting Shade",
        "4be506057006cc75b0181ff08183fba721bc09bcb1b28c3190c8eb5d72e2badb",
    ),
    (
        "dungeon_shade",
        "Dungeon Shade",
        "4be506057006cc75b0181ff08183fba721bc09bcb1b28c3190c8eb5d72e2badb",
    ),
    (
        "firefly",
        "Firefly",
        "e74575fc5da3990b1a3ad046658d95976f710451bdeb79ad7470464394f5ce04",
    ),
    (
        "furnace_whelp",
        "Furnace Whelp",
        "e74575fc5da3990b1a3ad046658d95976f710451bdeb79ad7470464394f5ce04",
    ),
    (
        "granite_gargoyle",
        "Granite Gargoyle",
        "1dff2b9395b5dedfbb5b83ef8540a52da605a1ae9ecf7345b4fcca3a40ddc0c2",
    ),
    (
        "hellkite_punisher",
        "Hellkite Punisher",
        "e74575fc5da3990b1a3ad046658d95976f710451bdeb79ad7470464394f5ce04",
    ),
    (
        "killer_bees",
        "Killer Bees",
        "b07ff66d090c90b9819c6ebd54e54f43bb8d39c17007db239274b3802c4caec0",
    ),
    (
        "marble_gargoyle",
        "Marble Gargoyle",
        "776b07ba668ed6bd74af70eaca23fddd12bccaa34662116a1108b837c53f190a",
    ),
    (
        "moltensteel_dragon",
        "Moltensteel Dragon",
        "ab9f8c4e41def56f47b24e40db2b8a593136db885042e7fb7fa6e1012138d12a",
    ),
    (
        "moonwing_moth",
        "Moonwing Moth",
        "776b07ba668ed6bd74af70eaca23fddd12bccaa34662116a1108b837c53f190a",
    ),
    (
        "tower_drake",
        "Tower Drake",
        "776b07ba668ed6bd74af70eaca23fddd12bccaa34662116a1108b837c53f190a",
    ),
];

#[test]
fn pump_self_batch_fingerprints_match_the_pinned_oracle_text() {
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
