//! Registry conformance for the reviewed enters-counter creature family.
//!
//! Every included card is a normal-layout creature whose entry trigger puts one
//! +1/+1 counter on a target creature. Card names, costs, types and bodies are
//! printed data; the shared scenario executes every card. Each fingerprint is
//! SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{Amount, EffectSubject, SpellEffectKind, TargetKind};
use tricerules_cards::{AbilityPresentation, Layout, TriggerCondition};

type Card = (
    &'static str,
    &'static str,
    &'static str,
    &'static [&'static str],
    Option<(u32, u32)>,
    &'static str,
);

const CARDS: &[Card] = &[
    (
        "bond_beetle",
        "Bond Beetle",
        "{G}",
        &["Creature", "Insect"],
        Some((0, 1)),
        "7173673f99794509e93eb6c089f3cfbda1166b1f651e6f0396a5402ce6c49f64",
    ),
    (
        "backup_agent",
        "Backup Agent",
        "{1}{W}",
        &["Creature", "Human", "Citizen"],
        Some((1, 1)),
        "7173673f99794509e93eb6c089f3cfbda1166b1f651e6f0396a5402ce6c49f64",
    ),
    (
        "dauntless_survivor",
        "Dauntless Survivor",
        "{1}{G}",
        &["Creature", "Human", "Warrior"],
        Some((1, 1)),
        "7173673f99794509e93eb6c089f3cfbda1166b1f651e6f0396a5402ce6c49f64",
    ),
    (
        "ironshell_beetle",
        "Ironshell Beetle",
        "{1}{G}",
        &["Creature", "Insect"],
        Some((1, 1)),
        "7173673f99794509e93eb6c089f3cfbda1166b1f651e6f0396a5402ce6c49f64",
    ),
    (
        "satyr_grovedancer",
        "Satyr Grovedancer",
        "{1}{G}",
        &["Creature", "Satyr", "Shaman"],
        Some((1, 1)),
        "7173673f99794509e93eb6c089f3cfbda1166b1f651e6f0396a5402ce6c49f64",
    ),
    (
        "timberland_guide",
        "Timberland Guide",
        "{1}{G}",
        &["Creature", "Human", "Scout"],
        Some((1, 1)),
        "7173673f99794509e93eb6c089f3cfbda1166b1f651e6f0396a5402ce6c49f64",
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
fn etb_counter_target_batch_maps_definitions() {
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
            body.map_or((None, None), |(p, t)| (Some(p), Some(t))),
            "{id} body"
        );

        assert!(f.keywords.is_empty(), "{id}");
        assert!(f.activated_abilities.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
        assert!(f.spell_effect.is_empty(), "{id}");

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
        let [SpellEffectKind::PutCounters {
            count: Amount::Fixed(1),
            subject: EffectSubject::Chosen(filter),
            ..
        }] = ability.effect.as_slice()
        else {
            panic!("{id}: one +1/+1 counter on a chosen creature");
        };
        assert_eq!(filter.kind, TargetKind::Creature, "{id}");

        let Some(targeting) = &ability.targeting else {
            panic!("{id}: one target group");
        };
        assert_eq!(targeting.groups.len(), 1, "{id}");
        assert_eq!(
            (targeting.groups[0].min, targeting.groups[0].max),
            (1, 1),
            "{id}"
        );
        assert_eq!(targeting.groups[0].effect_indices, [0], "{id}");
    }
}

#[test]
fn etb_counter_target_batch_fingerprints_match_the_pinned_oracle_text() {
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
