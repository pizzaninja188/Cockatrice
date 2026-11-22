//! Registry conformance for the reviewed Viashino Racketeer optional-discard family.
//!
//! Every included card is a normal-layout red creature with one entry trigger whose
//! authored program discards up to one card and draws only when the discard happened.
//! Card names, costs and stats are printed data; the shared scenario executes every
//! card. Each fingerprint is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::{AbilityPresentation, Layout, TriggerCondition};

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    u32,
    u32,
);

const CARDS: &[Card] = &[
    (
        "fissure_wizard",
        "Fissure Wizard",
        "{1}{R}",
        &["Creature", "Goblin", "Wizard"],
        2,
        1,
    ),
    (
        "immersturm_raider",
        "Immersturm Raider",
        "{1}{R}",
        &["Creature", "Demon", "Berserker"],
        2,
        1,
    ),
    (
        "viashino_racketeer",
        "Viashino Racketeer",
        "{2}{R}",
        &["Creature", "Lizard", "Rogue"],
        2,
        1,
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
fn viashino_racketeer_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, types, power, toughness) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        let f = face(registry, id);
        assert_eq!(f.face_id.as_str(), id, "{id} face id");
        assert_eq!(f.name, name, "{id} name");
        assert_eq!(f.mana_cost.to_string(), mana, "{id} mana cost");
        assert_eq!(f.types, types, "{id} types");
        assert_eq!(f.power, Some(power), "{id} power");
        assert_eq!(f.toughness, Some(toughness), "{id} toughness");
        assert!(f.keywords.is_empty(), "{id} has no printed keyword");
        assert!(
            f.activated_abilities.is_empty(),
            "{id} has no activated ability"
        );
        assert!(f.static_abilities.is_empty(), "{id} has no static ability");

        let [ability] = f.triggered_abilities.as_slice() else {
            panic!("{id}: exactly one entry trigger");
        };
        assert_eq!(ability.ability_id.as_str(), "triggered_01", "{id}");
        assert_eq!(
            ability.trigger,
            TriggerCondition::WhenSelfEntersBattlefield,
            "{id}"
        );
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id} presentation"
        );
        assert!(ability.effect.is_empty(), "{id}: using an authored program");
        let program = ability.program.as_ref().expect("{id}: program");
        assert_eq!(
            program.body.steps.len(),
            2,
            "{id}: discard then conditional draw"
        );
        assert_eq!(program.body.steps[0].id.as_str(), "discard_one", "{id}");
        assert_eq!(
            program.body.steps[1].id.as_str(),
            "draw_if_discarded",
            "{id}"
        );
    }
}

#[test]
fn viashino_racketeer_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, _, _, _, _) in CARDS {
        let expected = "535c5beae32336a4eb882f1331731e64c1440ac0b5099769465fbd098ce6febd";
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{id}\t")))
            .unwrap_or_else(|| panic!("missing fingerprint row for {id}"));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], id);
        assert_eq!(fields[1], name);
        assert_eq!(fields[2], id);
        assert_eq!(fields[3], name);
        assert_eq!(fields[4], expected, "fingerprint drift for {id}");
    }
}
