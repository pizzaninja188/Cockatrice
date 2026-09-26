use tricerules_cards::primitives::{
    AbilityCost, AbilitySourceZone, CastTriggerPlayer, CounterRemovalPaymentSource, EffectSubject,
    PlayerRecipient, SpellEffectKind, TriggerCondition,
};
use tricerules_cards::{CardRegistry, Color};

const KRAKEN_TOKEN: &str = "kraken_token_8_8";

#[test]
fn ominous_seas_registers_draw_foreshadow_cycling_and_kraken_activation() {
    let registry = CardRegistry::global();
    let card = registry.get("ominous_seas").expect("Ominous Seas");
    let face = card.primary_face();
    assert_eq!(face.name, "Ominous Seas");
    assert_eq!(face.mana_cost.to_string(), "{1}{U}");
    assert_eq!(face.types, ["Enchantment"]);

    let [trigger] = face.triggered_abilities.as_slice() else {
        panic!("Ominous Seas has one draw trigger");
    };
    assert_eq!(
        trigger.trigger,
        TriggerCondition::WheneverPlayerDrawsCard {
            drawer: CastTriggerPlayer::Controller,
        }
    );
    assert!(matches!(
        trigger.effect.as_slice(),
        [SpellEffectKind::PutCounters {
            counter,
            count: tricerules_cards::Amount::Fixed(1),
            subject: EffectSubject::Source,
        }] if counter.label() == "foreshadow"
    ));

    let [make_kraken, cycling] = face.activated_abilities.as_slice() else {
        panic!("Ominous Seas has one foreshadow activation and Cycling");
    };
    assert_eq!(make_kraken.source_zone, AbilitySourceZone::Battlefield);
    assert!(matches!(
        make_kraken.costs.as_slice(),
        [AbilityCost::RemoveCounters {
            counter: Some(counter),
            count: 8,
            payment_source: CounterRemovalPaymentSource::Source,
        }] if counter.label() == "foreshadow"
    ));
    assert!(matches!(
        make_kraken.effect.as_slice(),
        [SpellEffectKind::CreateTokens {
            token,
            count: tricerules_cards::Amount::Fixed(1),
            who: PlayerRecipient::Controller,
            ..
        }] if token == KRAKEN_TOKEN
    ));

    assert_eq!(cycling.source_zone, AbilitySourceZone::Hand);
    assert!(matches!(
        cycling.costs.as_slice(),
        [AbilityCost::Mana(cost), AbilityCost::DiscardSelf] if cost.to_string() == "{2}"
    ));
    assert!(matches!(
        cycling.effect.as_slice(),
        [SpellEffectKind::Draw {
            who: PlayerRecipient::Controller,
            count: tricerules_cards::Amount::Fixed(1),
        }]
    ));

    let token = registry.get(KRAKEN_TOKEN).expect("8/8 blue Kraken token");
    assert!(registry.is_token(KRAKEN_TOKEN));
    assert_eq!(token.name, "Kraken Token");
    let token_face = token.primary_face();
    assert_eq!(token_face.types, ["Creature", "Kraken"]);
    assert_eq!(token_face.colors_override.as_ref().unwrap(), &[Color::Blue]);
    assert_eq!((token_face.power, token_face.toughness), (Some(8), Some(8)));
}
