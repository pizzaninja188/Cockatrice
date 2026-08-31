use tricerules_cards::primitives::{
    EffectSubject, GameCondition, LifeAmount, PlayerRecipient, RelativePlayerSet, SpellEffectKind,
    TargetKind,
};
use tricerules_cards::{AbilityCost, ActivationCondition, ActivationTiming, CardRegistry, Keyword};

fn face(card_id: &str) -> &'static tricerules_cards::CardFace {
    let definition = CardRegistry::global()
        .get(card_id)
        .unwrap_or_else(|| panic!("{card_id} must be registered"));
    definition.primary_face()
}

#[test]
fn celestial_enforcer_requires_a_controlled_flying_creature() {
    let face = face("celestial_enforcer");
    assert_eq!(face.types, ["Creature", "Human", "Cleric"]);
    let ability = &face.activated_abilities[0];
    assert!(matches!(
        ability.costs.as_slice(),
        [AbilityCost::Mana(mana), AbilityCost::Tap] if mana.to_string() == "{1}{W}"
    ));
    assert_eq!(ability.timing, ActivationTiming::Normal);
    assert!(matches!(
        ability.conditions.as_slice(),
        [ActivationCondition::BattlefieldCreatureCount { filter, min: Some(1), max: None }]
            if filter.controllers == RelativePlayerSet::Controller
                && filter.required_keywords == [Keyword::Flying]
    ));
    assert!(matches!(
        ability.effect.as_slice(),
        [SpellEffectKind::Tap {
            subject: EffectSubject::Chosen(target),
        }] if target.kind == TargetKind::Creature
    ));
}

#[test]
fn goblin_bird_grabber_grants_flying_to_itself_without_a_target() {
    let face = face("goblin_bird-grabber");
    let ability = &face.activated_abilities[0];
    assert!(matches!(
        ability.costs.as_slice(),
        [AbilityCost::Mana(mana)] if mana.to_string() == "{R}"
    ));
    assert!(matches!(
        ability.conditions.as_slice(),
        [ActivationCondition::BattlefieldCreatureCount { filter, min: Some(1), max: None }]
            if filter.controllers == RelativePlayerSet::Controller
                && filter.required_keywords == [Keyword::Flying]
    ));
    assert_eq!(
        ability.effect,
        [SpellEffectKind::GrantKeywords {
            subject: EffectSubject::Source,
            keywords: vec![Keyword::Flying],
        }]
    );
}

#[test]
fn caged_zombie_uses_the_committed_turn_death_fact() {
    let face = face("caged_zombie");
    let ability = &face.activated_abilities[0];
    assert!(matches!(
        ability.conditions.as_slice(),
        [ActivationCondition::GameCondition(
            GameCondition::CreatureDeathsThisTurn {
                min: Some(1),
                max: None,
            }
        )]
    ));
    assert_eq!(
        ability.effect,
        [SpellEffectKind::LoseLife {
            amount: LifeAmount::Fixed(2),
            who: PlayerRecipient::EachOpponent,
        }]
    );
}
