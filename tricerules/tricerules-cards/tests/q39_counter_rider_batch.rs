//! Registry conformance for the queue-v39 counter-with-rider cohort.
use tricerules_cards::primitives::{Amount, SpellEffectKind};
use tricerules_cards::Layout;

struct Card {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    life: Option<u32>,
    scry: Option<u32>,
}

const CARDS: &[Card] = &[
    Card {
        id: "absorb",
        name: "Absorb",
        mana: "{W}{U}{U}",
        life: Some(3),
        scry: None,
    },
    Card {
        id: "fall_of_the_gavel",
        name: "Fall of the Gavel",
        mana: "{3}{W}{U}",
        life: Some(5),
        scry: None,
    },
    Card {
        id: "memory_drain",
        name: "Memory Drain",
        mana: "{2}{U}{U}",
        life: None,
        scry: Some(2),
    },
];

#[test]
fn q39_counter_rider_maps_definitions() {
    let registry = tricerules_cards::registry::global();
    for card in CARDS {
        let definition = registry
            .get(card.id)
            .unwrap_or_else(|| panic!("missing {}", card.id));
        assert_eq!(definition.layout, Layout::Normal, "{}", card.id);
        assert_eq!(registry.id_for_name(card.name), Some(card.id));
        let f = definition.primary_face();
        assert_eq!(f.face_id.as_str(), card.id);
        assert_eq!(f.name, card.name);
        assert_eq!(f.mana_cost.to_string(), card.mana);
        assert_eq!(f.types, ["Instant"]);
        assert!(f.keywords.is_empty());
        assert!(f.targeting.is_none());
        let [counter, rider] = f.spell_effect.as_slice() else {
            panic!("{}: counter then rider", card.id);
        };
        assert!(
            matches!(counter, SpellEffectKind::CounterTargetSpell { spell_filter, unless_controller_pays: None, .. } if spell_filter.card_type.is_none())
        );
        if let Some(life) = card.life {
            assert!(
                matches!(rider, SpellEffectKind::GainLife { amount: Amount::Fixed(n), .. } if *n == life)
            );
        }
        if let Some(scry) = card.scry {
            assert!(
                matches!(rider, SpellEffectKind::Scry { count: Amount::Fixed(n) } if *n == scry)
            );
        }
    }
}
