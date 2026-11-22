//! Registry conformance for the reviewed High-Rise Sawjack reach-and-block-pump family.
//!
//! Every included card is a normal-layout creature with Reach and one triggered ability
//! that pumps only itself when it blocks a creature with flying. Card names, costs,
//! printed characteristics and pump values are data; the shared scenario executes every
//! card. Each fingerprint is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{Keyword, SpellEffectKind, TriggerCondition};
use tricerules_cards::{AbilityPresentation, Layout};

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    i32,
    i32,
    i32,
    i32,
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "ezuris_archers",
        "Ezuri's Archers",
        "{G}",
        &["Creature", "Elf", "Archer"],
        1,
        2,
        3,
        0,
        "5cb789069c7f3d5e0eba94f8f4e87dfcd11861d3933f41f548223c9e099f02ff",
    ),
    (
        "high-rise_sawjack",
        "High-Rise Sawjack",
        "{2}{G}",
        &["Creature", "Elf", "Citizen"],
        2,
        3,
        2,
        0,
        "d23991d99918e6e469680b3e2537cde2269a78dda8ce778c7e9450819339e137",
    ),
    (
        "netcaster_spider",
        "Netcaster Spider",
        "{2}{G}",
        &["Creature", "Spider"],
        2,
        3,
        2,
        0,
        "d23991d99918e6e469680b3e2537cde2269a78dda8ce778c7e9450819339e137",
    ),
    (
        "woolly_spider",
        "Woolly Spider",
        "{1}{G}{G}",
        &["Creature", "Spider"],
        2,
        3,
        0,
        2,
        "676eb6e1c6f2c187fff3b580c6d21ef5b6c9b906946c7ed4c363749a99165c9c",
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
fn high_rise_sawjack_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, types, power, toughness, pump_power, pump_toughness, _) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        let f = face(registry, id);
        let face_id = if id == "high-rise_sawjack" {
            "high_rise_sawjack"
        } else {
            id
        };
        assert_eq!(f.face_id.as_str(), face_id, "{id} face id");
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(f.types, types, "{id} types");
        assert_eq!(f.power, Some(power as u32), "{id} power");
        assert_eq!(f.toughness, Some(toughness as u32), "{id} toughness");
        assert_eq!(f.keywords, vec![Keyword::Reach], "{id} reach");

        let [ability] = f.triggered_abilities.as_slice() else {
            panic!("{id}: one block trigger");
        };
        assert_eq!(ability.ability_id.as_str(), "triggered_01", "{id}");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{id}: line 2"
        );
        let TriggerCondition::WheneverSelfBlocksCreature { attacker } = &ability.trigger else {
            panic!("{id}: {:?}", ability.trigger);
        };
        assert!(
            attacker.required_keywords.contains(&Keyword::Flying),
            "{id}: flying-only filter"
        );
        assert!(
            !attacker.required_keywords.is_empty(),
            "{id}: a non-flying attacker must not trigger"
        );
        let [pump] = ability.effect.as_slice() else {
            panic!("{id}: one self pump");
        };
        assert!(
            matches!(
                pump,
                SpellEffectKind::PumpTarget {
                    power: printed_power,
                    toughness: printed_toughness,
                    subject: tricerules_cards::primitives::EffectSubject::Source,
                    ..
                } if *printed_power == pump_power && *printed_toughness == pump_toughness
            ),
            "{id}: {:?}",
            ability.effect
        );
        assert!(f.spell_effect.is_empty(), "{id}");
        assert!(f.activated_abilities.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
    }
}

#[test]
fn high_rise_sawjack_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, _, _, _, _, _, _, expected) in CARDS {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(
            fields[2],
            if id == "high-rise_sawjack" {
                "high_rise_sawjack"
            } else {
                id
            }
        );
        assert_eq!(fields[3], name);
        assert_eq!(fields[4], expected, "fingerprint drift for {id}");
    }
}
