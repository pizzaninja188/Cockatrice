mod common;

use common::FaceExpectation;
use tricerules_cards::primitives::{
    EffectSubject, LifeAmount, PermanentTypeFilter, PlayerRecipient, SpellEffectKind,
    TargetController, TargetKind,
};
use tricerules_cards::{CardRegistry, Keyword};

#[test]
fn issue_483_registers_complete_feed_the_swarm() {
    let registry = CardRegistry::global();
    assert_eq!(
        registry.id_for_name("Feed the Swarm"),
        Some("feed_the_swarm")
    );
    let face = FaceExpectation {
        id: "feed_the_swarm",
        name: "Feed the Swarm",
        face_id: "feed_the_swarm",
        mana_cost: "{1}{B}",
        types: &["Sorcery"],
        keywords: &[] as &[Keyword],
        power_toughness: None,
    }
    .check();
    let [SpellEffectKind::Destroy {
        subject: EffectSubject::Chosen(target),
    }, SpellEffectKind::LoseLife {
        amount: LifeAmount::TargetManaValue,
        who: PlayerRecipient::Controller,
    }] = face.spell_effect.as_slice()
    else {
        panic!("destroy followed by target-mana-value life loss");
    };
    assert_eq!(target.kind, TargetKind::AnyPermanent);
    assert_eq!(target.controller, TargetController::Opponent);
    assert_eq!(
        target.permanent_types,
        [
            PermanentTypeFilter::Creature,
            PermanentTypeFilter::Enchantment
        ]
    );
}
