//! Registry conformance for the reviewed "enters, draw a card, then discard a
//! card" family.

use tricerules_cards::primitives::{
    Amount, DiscardQuantity, InstructionKind, PlayerRecipient, SpellEffectKind, TriggerCondition,
};
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
        "elite_instructor",
        "Elite Instructor",
        "{2}{U}",
        &["Creature", "Human", "Wizard"],
        Some((2, 2)),
        "95a8e8723d3ffde5caf15a281f7dab2c93559b4974bfe260ae02758310b19742",
    ),
    (
        "merfolk_traders",
        "Merfolk Traders",
        "{1}{U}",
        &["Creature", "Merfolk"],
        Some((1, 2)),
        "95a8e8723d3ffde5caf15a281f7dab2c93559b4974bfe260ae02758310b19742",
    ),
    (
        "vodalian_merchant",
        "Vodalian Merchant",
        "{1}{U}",
        &["Creature", "Merfolk"],
        Some((1, 2)),
        "95a8e8723d3ffde5caf15a281f7dab2c93559b4974bfe260ae02758310b19742",
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
fn etb_loot_batch_maps_definitions() {
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
        assert_eq!(trigger.trigger, TriggerCondition::WhenSelfEntersBattlefield);
        assert!(trigger.effect.is_empty(), "{id}");
        let program = trigger.program.as_ref().expect("loot program");
        let [first, second] = program.body.steps.as_slice() else {
            panic!("{id}: draw then discard");
        };
        let InstructionKind::Perform(first_effect) = &first.kind else {
            panic!("{id}: draw instruction");
        };
        assert_eq!(
            first_effect.as_ref(),
            &SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(1),
            },
            "{id}"
        );
        let InstructionKind::Perform(second_effect) = &second.kind else {
            panic!("{id}: discard instruction");
        };
        assert_eq!(
            second_effect.as_ref(),
            &SpellEffectKind::Discard {
                who: PlayerRecipient::Controller,
                quantity: DiscardQuantity::Exact(1),
            },
            "{id}"
        );
    }
}

#[test]
fn etb_loot_batch_fingerprints_match_the_pinned_oracle_text() {
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
