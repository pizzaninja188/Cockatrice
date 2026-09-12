use tricerules_cards::primitives::{
    DrawDiscardOrder, EffectSubject, PlayerRecipient, SpellEffectKind, TargetFilter,
};
use tricerules_cards::{AbilityPresentation, Amount, CardRegistry, CounterKind, TriggerCondition};

#[test]
fn issue_259_registers_all_thirteen_generated_cards_with_exact_typed_etbs() {
    let registry = CardRegistry::global();

    for (id, ability_index, oracle_line) in [
        ("alanias_pathmaker", 0, 1),
        ("gundabad_opportunist", 0, 1),
        ("kulrath_zealot", 0, 1),
        ("crimson_operative", 1, 2),
    ] {
        let ability = &registry
            .get(id)
            .unwrap_or_else(|| panic!("missing {id}"))
            .primary_face()
            .triggered_abilities[ability_index];
        assert_etb_identity(ability, oracle_line);
        assert_eq!(
            ability.effect,
            [SpellEffectKind::ExileTopWithPlayPermission {
                player: PlayerRecipient::Controller,
                count: 1,
                count_by_cast_cost: None,
            }]
        );
    }

    for (id, oracle_line) in [
        ("jeong_jeongs_deserters", 1),
        ("cloudbound_moogle", 2),
        ("ironpaw_aspirant", 1),
    ] {
        let ability = &registry
            .get(id)
            .unwrap_or_else(|| panic!("missing {id}"))
            .primary_face()
            .triggered_abilities[0];
        assert_etb_identity(ability, oracle_line);
        assert_eq!(
            ability.effect,
            [SpellEffectKind::PutCounters {
                counter: CounterKind::PlusOnePlusOne,
                count: Amount::Fixed(1),
                subject: EffectSubject::Chosen(Box::new(TargetFilter::default_creature())),
            }]
        );
        let group = &ability.targeting.as_ref().expect("targeting").groups[0];
        assert_eq!((group.min, group.max), (1, 1));
        assert_eq!(group.effect_indices, [0]);
    }

    for (id, oracle_line) in [
        ("invasion_reinforcements", 2),
        ("treetop_freedom_fighters", 2),
        ("kyoshi_warriors", 1),
    ] {
        let ability = &registry
            .get(id)
            .unwrap_or_else(|| panic!("missing {id}"))
            .primary_face()
            .triggered_abilities[0];
        assert_etb_identity(ability, oracle_line);
        assert_eq!(
            ability.effect,
            [SpellEffectKind::CreateTokens {
                token: "ally_w_1_1".into(),
                count: Amount::Fixed(1),
                who: PlayerRecipient::Controller,
                tapped: false,
                sacrifice_timing: None,
            }]
        );
    }

    for (id, oracle_line) in [
        ("icewind_elemental", 2),
        ("temur_tawnyback", 1),
        ("bellowing_crier", 1),
    ] {
        let ability = &registry
            .get(id)
            .unwrap_or_else(|| panic!("missing {id}"))
            .primary_face()
            .triggered_abilities[0];
        assert_etb_identity(ability, oracle_line);
        assert_eq!(
            ability.effect,
            [SpellEffectKind::DrawDiscard {
                who: PlayerRecipient::Controller,
                draw_count: 1,
                discard_count: 1,
                order: DrawDiscardOrder::DrawThenDiscard,
                optional: false,
            }]
        );
    }
}

fn assert_etb_identity(ability: &tricerules_cards::TriggeredAbilityDef, oracle_line: u16) {
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![oracle_line])
    );
    assert_eq!(ability.trigger, TriggerCondition::WhenSelfEntersBattlefield);
    assert!(!ability.may);
}
