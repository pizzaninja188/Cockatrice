//! Registry conformance for the reviewed combat-damage draw family.

use tricerules_cards::primitives::{Amount, PlayerRecipient, SpellEffectKind, TriggerCondition};
use tricerules_cards::Layout;

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
        "nine-tail_white_fox",
        "Nine-Tail White Fox",
        "{2}{U}",
        &["Creature", "Fox", "Spirit"],
        Some((2, 2)),
        "ec03ee3ee160b0c95493395988afc795aab712133da58643742094a06c4a9bb2",
    ),
    (
        "soulknife_spy",
        "Soulknife Spy",
        "{2}{U}",
        &["Creature", "Elf", "Rogue"],
        Some((3, 2)),
        "ec03ee3ee160b0c95493395988afc795aab712133da58643742094a06c4a9bb2",
    ),
    (
        "stealer_of_secrets",
        "Stealer of Secrets",
        "{2}{U}",
        &["Creature", "Human", "Rogue"],
        Some((2, 2)),
        "ec03ee3ee160b0c95493395988afc795aab712133da58643742094a06c4a9bb2",
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
fn combat_damage_draw_batch_maps_definitions() {
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
        assert_eq!(
            (f.power, f.toughness),
            body.map_or((None, None), |(p, t)| (Some(p), Some(t))),
            "{id} body"
        );
        assert!(f.keywords.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");
        assert!(f.spell_effect.is_empty(), "{id}");
        assert!(f.targeting.is_none(), "{id}");
        assert!(f.activated_abilities.is_empty(), "{id}");
        let [trigger] = f.triggered_abilities.as_slice() else {
            panic!("{id}: exactly one triggered ability");
        };
        assert_eq!(
            trigger.trigger,
            TriggerCondition::WheneverSelfDealsCombatDamageToPlayer,
            "{id}"
        );
        assert_eq!(
            trigger.effect,
            [SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(1),
            }],
            "{id}"
        );
    }
}

#[test]
fn combat_damage_draw_batch_fingerprints_match_the_pinned_oracle_text() {
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
