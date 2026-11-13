//! Registry conformance for the reviewed optional basic-land search family.
//!
//! Every included card is a normal-layout creature whose only ability triggers when it
//! enters and runs a typed optional private basic-land search. Card names, costs, types and
//! bodies are printed data; the shared scenario executes every card. Each fingerprint is
//! SHA-256 of the exact pinned Oracle line.

use tricerules_cards::primitives::{
    CardTypeFilter, InstructionKind, SearchDestination, SpellEffectKind,
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
        "borderland_ranger",
        "Borderland Ranger",
        "{2}{G}",
        &["Creature", "Human", "Scout", "Ranger"],
        (2, 2),
        "ecfdc467a0274e5389710260c63a82c29b32a3fc106ebce02ade505fb456a708",
    ),
    (
        "civic_wayfinder",
        "Civic Wayfinder",
        "{2}{G}",
        &["Creature", "Elf", "Druid", "Warrior"],
        (2, 2),
        "ecfdc467a0274e5389710260c63a82c29b32a3fc106ebce02ade505fb456a708",
    ),
    (
        "skittering_surveyor",
        "Skittering Surveyor",
        "{3}",
        &["Artifact", "Creature", "Construct"],
        (1, 2),
        "ecfdc467a0274e5389710260c63a82c29b32a3fc106ebce02ade505fb456a708",
    ),
    (
        "sylvan_ranger",
        "Sylvan Ranger",
        "{1}{G}",
        &["Creature", "Elf", "Scout", "Ranger"],
        (1, 1),
        "ecfdc467a0274e5389710260c63a82c29b32a3fc106ebce02ade505fb456a708",
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
fn sylvan_ranger_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, types, body, _) in CARDS {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.layout, Layout::Normal, "{id}");
        assert_eq!(registry.id_for_name(name), Some(id), "{id}");
        let f = face(registry, id);
        assert_eq!(f.face_id.as_str(), id, "{id} face id");
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
        assert!(f.keywords.is_empty(), "{id}");
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
        assert_eq!(
            ability.trigger,
            TriggerCondition::WhenSelfEntersBattlefield,
            "{id}"
        );
        assert!(ability.effect.is_empty(), "{id}");
        let program = ability
            .program
            .as_ref()
            .expect("typed entry-search program");
        let [step] = program.body.steps.as_slice() else {
            panic!("{id}: one typed entry-search step");
        };
        assert!(
            matches!(
                &step.kind,
                InstructionKind::Choose { optional: true, branches, otherwise, .. }
                    if branches.len() == 1
                        && otherwise.steps.is_empty()
                        && matches!(branches[0].body.steps.as_slice(),
                            [step] if matches!(&step.kind,
                                InstructionKind::Perform(effect)
                                    if matches!(effect.as_ref(), SpellEffectKind::SearchLibrary {
                                        filter: Some(filter), destination: SearchDestination::Hand,
                                        shuffle: true, reveal: true, ..
                                    } if filter.card_type == Some(CardTypeFilter::BasicLand))))
            ),
            "{id}: optional reveal-search to hand with shuffle"
        );
    }
}

#[test]
fn sylvan_ranger_batch_fingerprints_match_the_pinned_oracle_text() {
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
        assert_eq!(fields[2], id);
        assert_eq!(fields[4], expected, "fingerprint drift for {id}");
    }
}
