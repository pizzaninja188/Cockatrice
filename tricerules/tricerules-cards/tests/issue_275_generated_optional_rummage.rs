use tricerules_cards::primitives::{DrawDiscardOrder, PlayerRecipient, SpellEffectKind};
use tricerules_cards::{AbilityPresentation, CardRegistry, TriggerCondition};

#[test]
fn issue_275_registers_the_two_generated_cards_with_exact_optional_etb() {
    let registry = CardRegistry::global();
    for (id, name, types, stats, reach, oracle_line) in [
        (
            "yuyan_archers",
            "Yuyan Archers",
            vec!["Creature", "Human", "Archer"],
            (3, 1),
            true,
            2,
        ),
        (
            "discerning_peddler",
            "Discerning Peddler",
            vec!["Creature", "Human", "Rogue"],
            (2, 2),
            false,
            1,
        ),
    ] {
        let definition = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.name, name);
        let face = definition.primary_face();
        assert_eq!(face.face_id.as_str(), id);
        assert_eq!(face.types, types);
        assert_eq!(face.power.zip(face.toughness), Some(stats));
        assert_eq!(
            face.keywords.contains(&tricerules_cards::Keyword::Reach),
            reach
        );
        let [ability] = face.triggered_abilities.as_slice() else {
            panic!("{id} must have exactly one generated trigger");
        };
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![oracle_line])
        );
        assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
        assert!(
            !ability.may,
            "optional discard belongs to the effect, not the trigger"
        );
        assert!(
            ability.targeting.is_none(),
            "discard/draw trigger has no targets"
        );
        assert_eq!(
            ability.effect,
            [SpellEffectKind::DrawDiscard {
                who: PlayerRecipient::Controller,
                draw_count: 1,
                discard_count: 1,
                order: DrawDiscardOrder::DiscardThenDraw,
                optional: true,
            }]
        );
    }
}
