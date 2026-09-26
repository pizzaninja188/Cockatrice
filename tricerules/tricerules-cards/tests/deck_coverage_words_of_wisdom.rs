use tricerules_cards::primitives::{Amount, PlayerRecipient, SpellEffectKind};
use tricerules_cards::{CardRegistry, Layout};

#[test]
fn words_of_wisdom_registers_its_ordered_player_draws() {
    let registry = CardRegistry::global();
    let card = registry
        .get("words_of_wisdom")
        .expect("Words of Wisdom registry definition");

    assert_eq!(card.name, "Words of Wisdom");
    assert_eq!(
        registry.id_for_name("Words of Wisdom"),
        Some("words_of_wisdom")
    );
    assert_eq!(card.layout, Layout::Normal);
    assert_eq!(card.face_count(), 1);

    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "words_of_wisdom");
    assert_eq!(face.mana_cost.to_string(), "{1}{U}");
    assert_eq!(face.types, ["Instant"]);
    assert!(face.activated_abilities.is_empty());
    assert!(face.triggered_abilities.is_empty());
    assert!(matches!(
        face.spell_effect.as_slice(),
        [
            SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(2),
            },
            SpellEffectKind::Draw {
                who: PlayerRecipient::EachOpponent,
                count: Amount::Fixed(1),
            },
        ]
    ));
}
