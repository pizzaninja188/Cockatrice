use tricerules_cards::primitives::{AbilityCost, SpellEffectKind, TargetChooser, TargetSchema};
use tricerules_cards::AbilityPresentation;

#[test]
fn arena_registers_its_complete_opponent_chosen_fight_ability() {
    let registry = tricerules_cards::registry::global();
    assert_eq!(registry.id_for_name("Arena"), Some("arena"));
    let card = registry.get("arena").expect("actual Arena is registered");
    assert_eq!(card.name, "Arena");
    assert_eq!(card.face_count(), 1);
    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "arena");
    assert_eq!(face.types, ["Land"]);
    assert_eq!(face.mana_cost.to_string(), "");
    assert!(face.colors().is_empty());
    let [ability] = face.activated_abilities.as_slice() else {
        panic!("Arena has one activated ability and no intrinsic mana ability");
    };
    assert_eq!(ability.ability_id.as_str(), "activated_01");
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    assert!(
        matches!(ability.costs.as_slice(), [AbilityCost::Mana(cost), AbilityCost::Tap] if cost.to_string() == "{3}")
    );
    assert!(matches!(
        ability.effect.as_slice(),
        [
            SpellEffectKind::Tap { .. },
            SpellEffectKind::Tap { .. },
            SpellEffectKind::Fight { .. }
        ]
    ));
    let schema = TargetSchema::compile(&ability.effect, ability.targeting.as_ref())
        .expect("Arena target roles");
    assert_eq!(schema.groups.len(), 2);
    assert_eq!(schema.groups[0].chooser, TargetChooser::Controller);
    assert_eq!(schema.groups[1].chooser, TargetChooser::ChosenOpponent);
    assert!(schema
        .groups
        .iter()
        .all(|group| group.min == 1 && group.max == 1));
    assert!(ability.mana_options().is_none());
    assert!(
        registry.get("magus_of_the_arena").is_none(),
        "reuse witness is private, not admitted by this batch"
    );
}
