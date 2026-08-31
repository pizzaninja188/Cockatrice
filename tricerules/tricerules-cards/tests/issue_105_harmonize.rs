use tricerules_cards::primitives::{Amount, SpellEffectKind};
use tricerules_cards::CardRegistry;

#[test]
fn issue_105_cards_publish_harmonize_costs_and_complete_effects() {
    let registry = CardRegistry::global();

    let whisper = registry.get("unending_whisper").expect("Unending Whisper");
    assert_eq!(
        whisper
            .primary_face()
            .harmonize_cost
            .as_ref()
            .map(ToString::to_string),
        Some("{5}{U}".to_string())
    );
    assert!(matches!(
        whisper.primary_face().spell_effect.as_slice(),
        [SpellEffectKind::Draw {
            count: Amount::Fixed(1),
            ..
        }]
    ));

    let bellow = registry.get("mammoth_bellow").expect("Mammoth Bellow");
    assert_eq!(
        bellow
            .primary_face()
            .harmonize_cost
            .as_ref()
            .map(ToString::to_string),
        Some("{5}{G}{U}{R}".to_string())
    );
    assert!(matches!(
        bellow.primary_face().spell_effect.as_slice(),
        [SpellEffectKind::CreateTokens { token, count, .. }]
            if token == "elephant_g_5_5" && *count == Amount::Fixed(1)
    ));

    let elephant = registry
        .get("elephant_g_5_5")
        .expect("5/5 green Elephant token");
    let face = elephant.primary_face();
    assert_eq!((face.power, face.toughness), (Some(5), Some(5)));
}
