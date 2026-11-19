//! Registry conformance for the queue-v39 draw-then-discard cohort.
use tricerules_cards::primitives::{
    Amount, DiscardQuantity, InstructionKind, PlayerRecipient, SpellEffectKind,
};
use tricerules_cards::Layout;

struct Card {
    id: &'static str,
    name: &'static str,
    mana: &'static str,
    draw: u32,
    discard: u32,
}

const CARDS: &[Card] = &[
    Card {
        id: "prying_eyes",
        name: "Prying Eyes",
        mana: "{4}{U}{U}",
        draw: 4,
        discard: 2,
    },
    Card {
        id: "thoughtflare",
        name: "Thoughtflare",
        mana: "{3}{U}{R}",
        draw: 4,
        discard: 2,
    },
    Card {
        id: "catalog",
        name: "Catalog",
        mana: "{2}{U}",
        draw: 2,
        discard: 1,
    },
];

#[test]
fn q39_draw_discard_maps_definitions() {
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
        assert!(f.spell_effect.is_empty());
        assert!(f.targeting.is_none());
        let program = f.spell_program.as_ref().expect("compiled program");
        let [draw, discard] = program.body.steps.as_slice() else {
            panic!("{}: draw then discard", card.id);
        };
        assert!(matches!(
            &draw.kind,
            InstructionKind::Perform(effect)
                if matches!(effect.as_ref(), SpellEffectKind::Draw {
                    who: PlayerRecipient::Controller, count: Amount::Fixed(n)
                } if *n == card.draw)
        ));
        assert!(matches!(
            &discard.kind,
            InstructionKind::Perform(effect)
                if matches!(effect.as_ref(), SpellEffectKind::Discard {
                    who: PlayerRecipient::Controller, quantity: DiscardQuantity::Exact(n)
                } if *n == card.discard)
        ));
    }
}
