//! Registry conformance for the reviewed Prowess creature family.
//!
//! Every included card is a normal-layout creature whose only printed ability
//! is Prowess, represented by the shared noncreature-cast trigger and
//! source-only +1/+1 pump. Card names, costs, types and bodies are printed
//! data; the shared scenario executes every card. Each fingerprint is SHA-256
//! of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{
    CardTypeFilter, CastTriggerPlayer, EffectSubject, SpellCastFilter, SpellEffectKind,
};
use tricerules_cards::{AbilityPresentation, Layout, TriggerCondition};

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    (u32, u32),
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "nimble-blade_khenra",
        "Nimble-Blade Khenra",
        "{1}{R}",
        &["Creature", "Jackal", "Warrior"],
        (1, 3),
        "bdfaacc6e6c4e6b44bbe0981484511438fafa14c57405d80011c6aa9914b6402",
    ),
    (
        "bloodfire_expert",
        "Bloodfire Expert",
        "{2}{R}",
        &["Creature", "Efreet", "Monk"],
        (3, 1),
        "bdfaacc6e6c4e6b44bbe0981484511438fafa14c57405d80011c6aa9914b6402",
    ),
    (
        "lightning_visionary",
        "Lightning Visionary",
        "{1}{R}",
        &["Creature", "Minotaur", "Shaman"],
        (2, 1),
        "bdfaacc6e6c4e6b44bbe0981484511438fafa14c57405d80011c6aa9914b6402",
    ),
    (
        "vedalken_blademaster",
        "Vedalken Blademaster",
        "{2}{U}",
        &["Creature", "Vedalken", "Soldier"],
        (2, 3),
        "bdfaacc6e6c4e6b44bbe0981484511438fafa14c57405d80011c6aa9914b6402",
    ),
    (
        "jeskai_student",
        "Jeskai Student",
        "{1}{W}",
        &["Creature", "Human", "Monk"],
        (1, 3),
        "bdfaacc6e6c4e6b44bbe0981484511438fafa14c57405d80011c6aa9914b6402",
    ),
    (
        "umara_entangler",
        "Umara Entangler",
        "{1}{U}",
        &["Creature", "Merfolk", "Rogue", "Ally"],
        (2, 1),
        "bdfaacc6e6c4e6b44bbe0981484511438fafa14c57405d80011c6aa9914b6402",
    ),
    (
        "sanguinary_mage",
        "Sanguinary Mage",
        "{1}{R}",
        &["Creature", "Vampire", "Wizard"],
        (1, 3),
        "bdfaacc6e6c4e6b44bbe0981484511438fafa14c57405d80011c6aa9914b6402",
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
fn prowess_creatures_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, types, body, _) in CARDS {
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
        assert!(f.supertypes.is_empty(), "{id} supertypes");
        assert_eq!(
            (f.power, f.toughness),
            (Some(body.0), Some(body.1)),
            "{id} body"
        );
        assert!(
            f.keywords.is_empty(),
            "{id}: keyword materialized as a trigger"
        );
        assert!(f.activated_abilities.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
        assert!(f.spell_effect.is_empty(), "{id}");
        assert!(f.targeting.is_none(), "{id}");

        let [ability] = f.triggered_abilities.as_slice() else {
            panic!("{id}: exactly one triggered ability");
        };
        assert_eq!(ability.ability_id.as_str(), "triggered_01", "{id}");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id} presentation"
        );
        assert!(
            matches!(
                &ability.trigger,
                TriggerCondition::WheneverPlayerCastsSpell {
                    caster: CastTriggerPlayer::Controller,
                    filter: SpellCastFilter {
                        card_type: Some(CardTypeFilter::Noncreature),
                        ..
                    },
                    ordinal: None,
                    ..
                }
            ),
            "{id}: controller noncreature cast trigger"
        );
        assert!(
            matches!(
                ability.effect.as_slice(),
                [SpellEffectKind::PumpTarget {
                    power: 1,
                    toughness: 1,
                    subject: EffectSubject::Source,
                    ..
                }]
            ),
            "{id}: +1/+1 source pump"
        );
        assert!(ability.targeting.is_none(), "{id}");
    }
}

#[test]
fn prowess_creatures_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, _, _, _, expected) in CARDS {
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
