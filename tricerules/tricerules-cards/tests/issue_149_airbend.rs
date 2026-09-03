use tricerules_cards::primitives::{Amount, EffectSubject, SpellEffectKind, TargetKind};
use tricerules_cards::{CardRegistry, ManaCost};

#[test]
fn airbending_lesson_uses_the_reusable_owner_cast_permission() {
    let card = CardRegistry::global()
        .get("airbending_lesson")
        .expect("Airbending Lesson");
    let face = card.primary_face();
    assert!(matches!(
        face.spell_effect.as_slice(),
        [
            SpellEffectKind::ExileWithOwnerCastPermission {
                subject: EffectSubject::Chosen(target),
                alternative_cost,
            },
            SpellEffectKind::Draw { count: Amount::Fixed(1), .. },
        ] if target.kind == TargetKind::AnyPermanent
            && target.excluded_permanent_types == [tricerules_cards::PermanentTypeFilter::Land]
            && *alternative_cost == ManaCost::parse("{2}").expect("cost")
    ));
}
