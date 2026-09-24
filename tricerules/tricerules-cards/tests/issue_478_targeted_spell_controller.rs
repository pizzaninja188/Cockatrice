mod common;

use common::FaceExpectation;
use tricerules_cards::primitives::{CardTypeFilter, PlayerRecipient, SpellEffectKind};

#[test]
fn issue_478_registers_complete_offer() {
    let face = FaceExpectation {
        id: "an_offer_you_cant_refuse",
        name: "An Offer You Can't Refuse",
        face_id: "an_offer_you_cant_refuse",
        mana_cost: "{U}",
        types: &["Instant"],
        keywords: &[],
        power_toughness: None,
    }
    .check();
    let [SpellEffectKind::CounterTargetSpell { spell_filter, .. }, SpellEffectKind::CreateTokens {
        token, count, who, ..
    }] = face.spell_effect.as_slice()
    else {
        panic!("noncreature counter followed by two Treasure for the targeted spell's controller");
    };
    assert_eq!(spell_filter.card_type, Some(CardTypeFilter::Noncreature));
    assert_eq!(token, "treasure");
    assert_eq!(*count, tricerules_cards::Amount::Fixed(2));
    assert_eq!(*who, PlayerRecipient::PreviousTargetedSpellController);
    assert!(SpellEffectKind::validate_list(&face.spell_effect[1..])
        .unwrap_err()
        .contains("immediately preceding CounterTargetSpell"));
}
