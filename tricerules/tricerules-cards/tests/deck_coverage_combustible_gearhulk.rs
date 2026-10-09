use tricerules_cards::primitives::{
    Amount, PlayerRecipient, SpellEffectKind, TargetKind, TriggerCondition,
};
use tricerules_cards::{AbilityPresentation, Keyword, Layout, ManaCost};

#[test]
fn combustible_gearhulk_registers_its_exact_identity_and_etb_choice() {
    let card = tricerules_cards::registry::global()
        .get("combustible_gearhulk")
        .expect("Combustible Gearhulk needs a complete definition");
    assert_eq!(card.name, "Combustible Gearhulk");
    assert_eq!(card.layout, Layout::Normal);
    assert_eq!(card.face_count(), 1);

    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "combustible_gearhulk");
    assert_eq!(face.mana_cost, ManaCost::parse("{4}{R}{R}").unwrap());
    assert_eq!(face.types, ["Artifact", "Creature", "Construct"]);
    assert_eq!((face.power, face.toughness), (Some(6), Some(6)));
    assert!(face.keywords.contains(&Keyword::FirstStrike));

    let [trigger] = face.triggered_abilities.as_slice() else {
        panic!("Combustible Gearhulk has exactly one triggered ability");
    };
    assert_eq!(trigger.ability_id.as_str(), "triggered_01");
    assert!(matches!(
        trigger.presentation,
        AbilityPresentation::OracleLines(ref lines) if lines == &[2]
    ));
    assert_eq!(trigger.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert!(matches!(
        trigger.effect.as_slice(),
        [SpellEffectKind::ChooseResolutionBranch {
            chooser: PlayerRecipient::TargetedPlayer {
                group_index: 0,
                kind: TargetKind::OpponentPlayer,
            },
            optional: true,
            branches,
            otherwise,
            ..
        }] if matches!(
            branches.as_slice(),
            [branch] if matches!(
                branch.effects.as_slice(),
                [SpellEffectKind::Draw { count: Amount::Fixed(3), who: PlayerRecipient::Controller }]
            )
        ) && matches!(
            otherwise.as_slice(),
            [SpellEffectKind::Mill { count: Amount::Fixed(3), who: PlayerRecipient::Controller },
             SpellEffectKind::DamagePlayer {
                 who: PlayerRecipient::TargetedPlayer { group_index: 0, kind: TargetKind::OpponentPlayer },
                 amount: Amount::Count(tricerules_cards::primitives::CountExpression::PreviousMillManaValueSum),
             }]
        )
    ));
}
