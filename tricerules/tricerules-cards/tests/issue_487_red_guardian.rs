use tricerules_cards::primitives::{EffectSubject, TargetController, TargetKind};
use tricerules_cards::{CardRegistry, SpellEffectKind, TriggerCondition};

#[test]
fn red_guardian_is_a_complete_registered_standard_card() {
    let card = CardRegistry::global()
        .get("red_guardian,_super-soldier")
        .expect("Red Guardian, Super-Soldier should be admitted as a complete card");
    let face = card.primary_face();

    assert_eq!(card.name, "Red Guardian, Super-Soldier");
    assert_eq!(face.mana_cost.to_string(), "{2}{W}");
    assert_eq!(face.supertypes, ["Legendary"]);
    assert_eq!(face.types, ["Creature", "Human", "Soldier", "Villain"]);
    assert_eq!((face.power, face.toughness), (Some(2), Some(2)));
    assert!(face.keywords.contains(&tricerules_cards::Keyword::Flash));

    let [trigger] = face.triggered_abilities.as_slice() else {
        panic!("Red Guardian should have one enters trigger");
    };
    assert_eq!(trigger.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert!(matches!(
        trigger.effect.as_slice(),
        [SpellEffectKind::Destroy {
            subject: EffectSubject::Chosen(filter),
        }] if filter.kind == TargetKind::Creature
            && filter.controller == TargetController::Opponent
            && filter.dealt_damage_this_turn == Some(true)
    ));

    let [group] = trigger
        .targeting
        .as_ref()
        .expect("target declaration")
        .groups
        .as_slice()
    else {
        panic!("Red Guardian should target exactly one creature");
    };
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(group.effect_indices, [0]);

    let fingerprint = include_str!("../presentation/oracle_fingerprints.tsv")
        .lines()
        .find(|row| row.starts_with("red_guardian,_super-soldier\t"))
        .expect("pinned Oracle presentation fingerprint");
    assert_eq!(
        fingerprint,
        "red_guardian,_super-soldier\tRed Guardian, Super-Soldier\tred_guardian_super_soldier\tRed Guardian, Super-Soldier\t39b8a9fc752175d18171463417a7ab15f32e53d2895d675f736726de25ae89ec"
    );
}
