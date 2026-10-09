use tricerules_cards::primitives::{
    CastTriggerPlayer, MassPlayerSet, PermanentTypeFilter, SpellEffectKind, TargetKind,
};
use tricerules_cards::{AbilityPresentation, Layout, TriggerCondition};

#[test]
fn nature_s_will_registers_its_grouped_combat_damage_ability() {
    let registry = tricerules_cards::registry::global();
    assert_eq!(registry.id_for_name("Nature's Will"), Some("natures_will"));
    let card = registry.get("natures_will").expect("Nature's Will");
    assert_eq!(card.name, "Nature's Will");
    assert_eq!(card.layout, Layout::Normal);
    assert_eq!(card.face_count(), 1);

    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "natures_will");
    assert_eq!(face.mana_cost.to_string(), "{2}{G}{G}");
    assert_eq!(face.types, ["Enchantment"]);
    assert!(face.spell_effect.is_empty());
    assert!(face.static_abilities.is_empty());
    assert!(face.activated_abilities.is_empty());

    let [trigger] = face.triggered_abilities.as_slice() else {
        panic!("Nature's Will has exactly one triggered ability");
    };
    assert_eq!(trigger.ability_id.as_str(), "triggered_01");
    assert!(matches!(
        &trigger.presentation,
        AbilityPresentation::OracleLines(lines) if lines == &[1]
    ));
    assert_eq!(
        trigger.trigger,
        TriggerCondition::WheneverCreatureDealsCombatDamageToPlayer {
            source_controller: CastTriggerPlayer::Controller,
            damaged_player: CastTriggerPlayer::AnyPlayer,
            cardinality:
                tricerules_cards::CombatDamageTriggerCardinality::OneOrMorePerDamagedPlayer,
        }
    );
    assert!(matches!(
        trigger.effect.as_slice(),
        [
            SpellEffectKind::TapAll {
                players: MassPlayerSet::AffectedPlayer,
                filter: tap_filter,
            },
            SpellEffectKind::UntapAll {
                players: MassPlayerSet::Controller,
                filter: untap_filter,
            },
        ] if tap_filter.kind == TargetKind::AnyPermanent
            && tap_filter.permanent_types == [PermanentTypeFilter::Land]
            && untap_filter == tap_filter
    ));
}
