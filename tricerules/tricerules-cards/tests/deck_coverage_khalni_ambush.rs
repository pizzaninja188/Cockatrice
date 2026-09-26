use tricerules_cards::primitives::{
    AbilityCost, AbilitySourceZone, EntersTappedAffected, SpellEffectKind, StaticAbilityDef,
};
use tricerules_cards::{CardRegistry, Layout};

#[test]
fn khalni_ambush_and_khalni_territory_register_both_faces() {
    let registry = CardRegistry::global();
    let card = registry
        .get("khalni_ambush_khalni_territory")
        .expect("Khalni Ambush // Khalni Territory");

    assert_eq!(card.name, "Khalni Ambush // Khalni Territory");
    assert_eq!(card.layout, Layout::ModalDfc);
    assert_eq!(card.face_count(), 2);

    let front = card.face(0).expect("Khalni Ambush front face");
    assert_eq!(front.face_id.as_str(), "khalni_ambush");
    assert_eq!(front.name, "Khalni Ambush");
    assert_eq!(front.mana_cost.to_string(), "{2}{G}");
    assert_eq!(front.types, ["Instant"]);
    assert!(matches!(
        front.spell_effect.as_slice(),
        [SpellEffectKind::Fight { .. }]
    ));
    let targeting = front.targeting.as_ref().expect("two creature targets");
    let [you_control, not_you_control] = targeting.groups.as_slice() else {
        panic!("Khalni Ambush has two target groups");
    };
    assert_eq!((you_control.min, you_control.max), (1, 1));
    assert_eq!(you_control.prompt, "Choose target creature you control");
    assert_eq!((not_you_control.min, not_you_control.max), (1, 1));
    assert_eq!(
        not_you_control.prompt,
        "Choose target creature you don't control"
    );
    assert_eq!(not_you_control.distinct_from, [0]);

    let back = card.face(1).expect("Khalni Territory back face");
    assert_eq!(back.face_id.as_str(), "khalni_territory");
    assert_eq!(back.name, "Khalni Territory");
    assert_eq!(back.mana_cost.to_string(), "");
    assert_eq!(back.types, ["Land"]);
    assert!(matches!(
        back.static_abilities.as_slice(),
        [ability] if matches!(
            ability.definition,
            StaticAbilityDef::EntersTapped {
            affected: EntersTappedAffected::Self_,
            condition: None,
            unless_cost: None,
            }
        )
    ));
    let [mana] = back.activated_abilities.as_slice() else {
        panic!("Khalni Territory has one mana ability");
    };
    assert_eq!(mana.source_zone, AbilitySourceZone::Battlefield);
    assert!(matches!(mana.costs.as_slice(), [AbilityCost::Tap]));
    assert!(matches!(
        mana.effect.as_slice(),
        [SpellEffectKind::ProduceMana { options, .. }]
            if options.len() == 1 && options[0].g == 1
    ));
}
