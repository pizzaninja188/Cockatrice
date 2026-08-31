use tricerules_cards::primitives::{
    Amount, CreatureScopeController, CreatureScopeFilter, PlayerRecipient, SpellEffectKind,
};
use tricerules_cards::CardRegistry;

#[test]
fn opponent_mass_scope_cards_have_complete_oracle_behavior() {
    let registry = CardRegistry::global();

    let chill = registry
        .get("uncomfortable_chill")
        .expect("Uncomfortable Chill must be registered");
    let chill_face = chill.primary_face();
    assert_eq!(chill.name, "Uncomfortable Chill");
    assert_eq!(chill_face.mana_cost.to_string(), "{2}{U}");
    assert_eq!(chill_face.types, ["Instant"]);
    assert_eq!(
        chill_face.spell_effect,
        [
            SpellEffectKind::PumpAll {
                filter: CreatureScopeFilter {
                    controller: Some(CreatureScopeController::Opponents),
                    ..CreatureScopeFilter::default()
                },
                power: -2,
                toughness: 0,
            },
            SpellEffectKind::Draw {
                who: PlayerRecipient::Controller,
                count: Amount::Fixed(1),
            },
        ]
    );

    let obsolete = registry
        .get("make_obsolete")
        .expect("Make Obsolete must be registered");
    let obsolete_face = obsolete.primary_face();
    assert_eq!(obsolete.name, "Make Obsolete");
    assert_eq!(obsolete_face.mana_cost.to_string(), "{2}{B}");
    assert_eq!(obsolete_face.types, ["Instant"]);
    assert_eq!(
        obsolete_face.spell_effect,
        [SpellEffectKind::PumpAll {
            filter: CreatureScopeFilter {
                controller: Some(CreatureScopeController::Opponents),
                ..CreatureScopeFilter::default()
            },
            power: -1,
            toughness: -1,
        }]
    );
}
