//! Registry conformance for the reviewed Soothsayer Adept draw-then-discard family.
//!
//! Every included card is a normal-layout creature with one activated ability whose
//! costs are printed mana plus Tap and whose Program draws one card, then discards one
//! card. Card names, costs, bodies and types are data; the shared scenario executes
//! every card. Each fingerprint is SHA-256 of the exact pinned normalized Oracle text.

use tricerules_cards::primitives::{
    DiscardQuantity, InstructionKind, PlayerRecipient, SpellEffectKind,
};
use tricerules_cards::{AbilityPresentation, Amount, Layout};

type Card = (
    &'static str,
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
        "bloodfire_mentor",
        "Bloodfire Mentor",
        "{2}{R}",
        "{2}{U}",
        &["Creature", "Efreet", "Shaman"],
        0,
        5,
        "ed24a7323fd9ed5fd31bb766a26eb8d401274d661ac69f59b1a15a1a11811afa",
    ),
    (
        "erratic_visionary",
        "Erratic Visionary",
        "{1}{U}",
        "{1}{U}",
        &["Creature", "Human", "Wizard"],
        1,
        3,
        "45847ab7d7d32f2ec6628fcaa89054efdc2a131a36d179a2305c2119f5cd402f",
    ),
    (
        "research_assistant",
        "Research Assistant",
        "{1}{U}",
        "{3}{U}",
        &["Creature", "Human", "Wizard"],
        1,
        3,
        "486c64ae7d0f98464bd594363e00a47238ee1e75cc9e991ddff66e109da6d029",
    ),
    (
        "soothsayer_adept",
        "Soothsayer Adept",
        "{1}{U}",
        "{1}{U}",
        &["Creature", "Merfolk", "Wizard"],
        1,
        3,
        "45847ab7d7d32f2ec6628fcaa89054efdc2a131a36d179a2305c2119f5cd402f",
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
fn soothsayer_adept_batch_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for &(id, name, mana, activation, types, power, toughness, _) in CARDS {
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
        assert!(f.keywords.is_empty(), "{id} has no keyword");
        assert!(f.triggered_abilities.is_empty(), "{id}");
        assert!(f.static_abilities.is_empty(), "{id}");

        let [ability] = f.activated_abilities.as_slice() else {
            panic!("{id}: one activated ability");
        };
        assert_eq!(ability.ability_id.as_str(), "activated_01", "{id}");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id}: line 1"
        );
        assert_eq!(ability.costs.len(), 2, "{id}: mana and tap");
        assert!(
            matches!(&ability.costs[0], tricerules_cards::primitives::AbilityCost::Mana(cost)
                if cost.to_string() == activation),
            "{id}: {:?}",
            ability.costs
        );
        assert!(
            matches!(
                &ability.costs[1],
                tricerules_cards::primitives::AbilityCost::Tap
            ),
            "{id}: tap cost"
        );
        assert!(ability.effect.is_empty(), "{id}: program only");
        let program = ability.program.as_ref().expect("{id}: program");
        let [draw, discard] = program.body.steps.as_slice() else {
            panic!("{id}: exact draw then discard");
        };
        assert!(
            matches!(
                &draw.kind,
                InstructionKind::Perform(effect)
                    if matches!(
                        effect.as_ref(),
                        SpellEffectKind::Draw { count, who }
                            if *count == Amount::Fixed(1)
                                && *who == PlayerRecipient::Controller
                    )
            ),
            "{id}: {:?}",
            draw.kind
        );
        assert!(
            matches!(
                &discard.kind,
                InstructionKind::Perform(effect)
                    if matches!(
                        effect.as_ref(),
                        SpellEffectKind::Discard { quantity, .. }
                            if *quantity == DiscardQuantity::Exact(1)
                    )
            ),
            "{id}: {:?}",
            discard.kind
        );
    }
}

#[test]
fn soothsayer_adept_batch_fingerprints_match_the_pinned_oracle_text() {
    let fingerprints = include_str!("../presentation/oracle_fingerprints.tsv");
    for &(id, name, _, _, _, _, _, expected) in CARDS {
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
