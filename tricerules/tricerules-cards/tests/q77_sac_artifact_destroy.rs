//! Registry conformance for the queue-v77 03-sac-artifact-destroy cohort.
//!
//! Every card has one activated ability with a printed mana cost plus sacrificing the source, and
//! destroys a targeted artifact. Fingerprints are checked after the presentation metadata refresh.

use tricerules_cards::primitives::{
    AbilityCost, EffectSubject, PermanentTypeFilter, SpellEffectKind, TargetChooser, TargetFilter,
    TargetGroupDef, TargetKind, TargetingDef,
};
use tricerules_cards::{AbilityPresentation, Layout};

struct Card {
    id: &'static str,
    name: &'static str,
    mana_cost: &'static str,
    types: &'static [&'static str],
    power: u32,
    toughness: u32,
    activation_mana: &'static str,
    fingerprint: &'static str,
}

const CARDS: &[Card] = &[
    Card {
        id: "goblin_replica",
        name: "Goblin Replica",
        mana_cost: "{3}",
        types: &["Artifact", "Creature", "Goblin"],
        power: 2,
        toughness: 2,
        activation_mana: "{3}{R}",
        fingerprint: "fa2c9246ab7f74d80c7f9d593a74bad87fb24ef9511fc7172059bc464b4607e1",
    },
    Card {
        id: "torch_fiend",
        name: "Torch Fiend",
        mana_cost: "{1}{R}",
        types: &["Creature", "Devil"],
        power: 2,
        toughness: 1,
        activation_mana: "{R}",
        fingerprint: "67396cbde98dac8aa8884c696c9c24425b07a6ece749a8e144a1e50ecacb2941",
    },
    Card {
        id: "reckless_reveler",
        name: "Reckless Reveler",
        mana_cost: "{1}{R}",
        types: &["Creature", "Satyr"],
        power: 2,
        toughness: 1,
        activation_mana: "{R}",
        fingerprint: "67396cbde98dac8aa8884c696c9c24425b07a6ece749a8e144a1e50ecacb2941",
    },
    Card {
        id: "ruinous_gremlin",
        name: "Ruinous Gremlin",
        mana_cost: "{R}",
        types: &["Creature", "Gremlin"],
        power: 1,
        toughness: 1,
        activation_mana: "{2}{R}",
        fingerprint: "0a27e834694c1bc32599882494c7751e1a715dc461141bed0ea1e8f9b9b915a2",
    },
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
fn q77_sac_artifact_destroy_map_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in CARDS {
        let definition = registry
            .get(card.id)
            .unwrap_or_else(|| panic!("missing {}", card.id));
        assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
        assert_eq!(
            registry.id_for_name(card.name),
            Some(card.id),
            "{}",
            card.id
        );
        let f = face(registry, card.id);
        assert_eq!(f.face_id.as_str(), card.id, "{} face id", card.id);
        assert_eq!(f.name, card.name, "{} name", card.id);
        assert_eq!(f.mana_cost.to_string(), card.mana_cost, "{} mana", card.id);
        assert_eq!(f.types, card.types, "{} types", card.id);
        assert_eq!(
            (f.power, f.toughness),
            (Some(card.power), Some(card.toughness)),
            "{} body",
            card.id
        );
        assert!(f.keywords.is_empty(), "{}", card.id);
        assert!(f.triggered_abilities.is_empty(), "{}", card.id);
        assert!(f.static_abilities.is_empty(), "{}", card.id);
        assert!(f.spell_effect.is_empty(), "{}", card.id);

        let [ability] = f.activated_abilities.as_slice() else {
            panic!("{}: exactly one activated ability", card.id);
        };
        assert_eq!(ability.ability_id.as_str(), "activated_01", "{}", card.id);
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{}",
            card.id
        );
        assert_eq!(ability.costs.len(), 2, "{}", card.id);
        assert!(
            matches!(&ability.costs[0], AbilityCost::Mana(cost) if cost.to_string() == card.activation_mana),
            "{} activation mana",
            card.id
        );
        assert_eq!(ability.costs[1], AbilityCost::SacrificeSelf, "{}", card.id);

        let [effect] = ability.effect.as_slice() else {
            panic!("{}: exactly one effect", card.id);
        };
        match effect {
            SpellEffectKind::Destroy {
                subject: EffectSubject::Chosen(filter),
            } => {
                assert_eq!(
                    &**filter,
                    &TargetFilter {
                        kind: TargetKind::AnyPermanent,
                        permanent_types: vec![PermanentTypeFilter::Artifact],
                        ..Default::default()
                    },
                    "{} target filter",
                    card.id
                );
            }
            other => panic!("{}: expected Destroy(Chosen), got {other:?}", card.id),
        }

        let targeting = ability
            .targeting
            .as_ref()
            .unwrap_or_else(|| panic!("{} targeting", card.id));
        assert_eq!(
            *targeting,
            TargetingDef {
                groups: vec![TargetGroupDef {
                    for_each_opponent: false,
                    chooser: TargetChooser::Controller,
                    controlled_by_chooser: false,
                    min: 1,
                    max: 1,
                    prompt: "Choose target artifact".to_string(),
                    effect_indices: vec![0],
                    distinct_from: Vec::new(),
                    relation: None,
                    cast_cost_expansion: None,
                }],
            },
            "{} targeting",
            card.id
        );
    }
}

#[test]
fn q77_sac_artifact_destroy_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for card in CARDS {
        let row = fingerprints
            .lines()
            .find(|line| line.starts_with(&format!("{}\t", card.id)))
            .unwrap_or_else(|| panic!("missing fingerprint row for {}", card.id));
        let fields: Vec<&str> = row.split('\t').collect();
        assert_eq!(fields.len(), 5, "fingerprint row shape: {row}");
        assert_eq!(fields[0], card.id);
        assert_eq!(fields[1], card.name);
        assert_eq!(fields[2], card.id, "face id drift for {}", card.id);
        assert_eq!(
            fields[4], card.fingerprint,
            "fingerprint drift for {}",
            card.id
        );
    }
}
