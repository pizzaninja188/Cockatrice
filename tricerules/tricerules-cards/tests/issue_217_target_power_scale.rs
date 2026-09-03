use tricerules_cards::{
    AbilityPresentation, CardRegistry, CastTriggerPlayer, PermanentTypeFilter,
    PowerToughnessCharacteristic, PtScaleBasis, SpellEffectKind, TriggerCondition,
};

#[test]
fn issue_217_target_relative_power_scale_loads() {
    let registry = CardRegistry::from_chunks_and_tokens(
        &[r#"
        (
          id: "double_power_test",
          name: "Double Power Test",
          face_id: "double_power_test",
          mana_cost: "{G}",
          types: ["Instant"],
          spell_effect: [PumpTarget(
            power: 0,
            toughness: 0,
            scale: Some((
              basis: Subject(Power),
              power_per_unit: 1,
              toughness_per_unit: 0,
            )),
            subject: Chosen((kind: Creature)),
          )],
        )
    "#],
        &[],
    )
    .expect("target-relative P/T scale should load");

    let [SpellEffectKind::PumpTarget {
        scale: Some(scale), ..
    }] = registry
        .get("double_power_test")
        .expect("fixture")
        .primary_face()
        .spell_effect
        .as_slice()
    else {
        panic!("target-relative pump shape");
    };
    assert_eq!(
        scale.basis,
        PtScaleBasis::Subject(PowerToughnessCharacteristic::Power)
    );
}

#[test]
fn issue_217_rejects_a_scale_that_cannot_change_power_or_toughness() {
    let result = CardRegistry::from_chunks_and_tokens(
        &[r#"
            (
              id: "empty_scale_test",
              name: "Empty Scale Test",
              face_id: "empty_scale_test",
              mana_cost: "{G}",
              types: ["Instant"],
              spell_effect: [PumpTarget(
                power: 1,
                toughness: 0,
                scale: Some((
                  basis: Subject(Power),
                  power_per_unit: 0,
                  toughness_per_unit: 0,
                )),
                subject: Chosen((kind: Creature)),
              )],
            )
        "#],
        &[],
    );

    assert!(result.is_err());
}

#[test]
fn issue_217_mightform_harmonizer_has_exact_landfall_shape() {
    let card = CardRegistry::global()
        .get("mightform_harmonizer")
        .expect("Mightform Harmonizer");
    let face = card.primary_face();

    assert_eq!(card.name, "Mightform Harmonizer");
    assert_eq!(face.face_id.as_str(), "mightform_harmonizer");
    assert_eq!(face.mana_cost.to_string(), "{2}{G}{G}");
    assert_eq!(
        face.warp_cost.as_ref().map(ToString::to_string),
        Some("{2}{G}".into())
    );
    assert_eq!(face.types, ["Creature", "Insect", "Druid"]);
    assert_eq!((face.power, face.toughness), (Some(4), Some(4)));

    let [trigger] = face.triggered_abilities.as_slice() else {
        panic!("one landfall ability");
    };
    assert_eq!(trigger.ability_id.as_str(), "triggered_01");
    assert_eq!(
        trigger.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    let TriggerCondition::WheneverPermanentEntersBattlefield {
        controller,
        filter,
        creature_filter,
    } = &trigger.trigger
    else {
        panic!("landfall trigger");
    };
    assert_eq!(*controller, CastTriggerPlayer::Controller);
    assert_eq!(filter.permanent_type, Some(PermanentTypeFilter::Land));
    assert!(creature_filter.is_none());

    let [SpellEffectKind::PumpTarget {
        power,
        toughness,
        scale: Some(scale),
        ..
    }] = trigger.effect.as_slice()
    else {
        panic!("targeted power scaling effect");
    };
    assert_eq!((*power, *toughness), (0, 0));
    assert_eq!(
        scale.basis,
        PtScaleBasis::Subject(PowerToughnessCharacteristic::Power)
    );
    assert_eq!((scale.power_per_unit, scale.toughness_per_unit), (1, 0));
}
