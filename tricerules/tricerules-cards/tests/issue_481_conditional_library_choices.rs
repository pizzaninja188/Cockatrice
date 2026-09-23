mod common;

use common::FaceExpectation;
use tricerules_cards::primitives::{
    Amount, BattlefieldAggregate, CardTypeFilter, EffectSubject, GameCondition,
    LibraryPartitionKind, PermanentTypeFilter, RelativePlayerSet, SpellEffectKind, TargetKind,
};
use tricerules_cards::{CardFace, CardRegistry, Keyword};

const TAKEN_BY_NIGHTMARES_DRAFT: &str = r#"
(
  id: "taken_by_nightmares",
  name: "Taken by Nightmares",
  face_id: "taken_by_nightmares",
  mana_cost: "{2}{B}{B}",
  types: ["Instant"],
  spell_effect: [
    Conditional(
      condition: BattlefieldAggregate(
        filter: (controllers: Controller, card_type: Some(Enchantment)),
        aggregate: Count,
        min: Some(1),
      ),
      effect: Scry(count: 2),
    ),
  ],
)
"#;

const FAILED_FORDING_DRAFT: &str = r#"
(
  id: "failed_fording",
  name: "Failed Fording",
  face_id: "failed_fording",
  mana_cost: "{1}{U}",
  types: ["Instant"],
  spell_effect: [
    Conditional(
      condition: BattlefieldAggregate(
        filter: (
          controllers: Controller,
          card_type: Some(Land),
          required_subtypes: ["Desert"],
        ),
        aggregate: Count,
        min: Some(1),
      ),
      effect: LibraryPartition(
        count: 1,
        top_min: 0,
        kind: Surveil,
      ),
    ),
  ],
)
"#;

#[test]
fn issue_481_conditional_scry_and_surveil_are_valid_typed_effects() {
    for (card, draft) in [
        ("Taken by Nightmares", TAKEN_BY_NIGHTMARES_DRAFT),
        ("Failed Fording", FAILED_FORDING_DRAFT),
    ] {
        CardRegistry::from_authoring_draft(draft)
            .unwrap_or_else(|error| panic!("{card} typed draft must validate: {error}"));
    }
}

#[test]
fn issue_481_does_not_admit_unneeded_conditional_library_look() {
    let unsupported = FAILED_FORDING_DRAFT.replace("kind: Surveil", "kind: Look");
    let error = CardRegistry::from_authoring_draft(&unsupported)
        .expect_err("Conditional Look has no demonstrated issue-481 card use");
    assert!(error.to_string().contains("Conditional currently supports"));
}

fn face(id: &str) -> &'static CardFace {
    CardRegistry::global()
        .get(id)
        .unwrap_or_else(|| panic!("missing complete issue-481 card {id}"))
        .primary_face()
}

#[test]
fn issue_481_registers_both_complete_cards_and_exact_conditional_effects() {
    for (id, name, mana_cost) in [
        ("taken_by_nightmares", "Taken by Nightmares", "{2}{B}{B}"),
        ("failed_fording", "Failed Fording", "{1}{U}"),
    ] {
        assert_eq!(CardRegistry::global().id_for_name(name), Some(id));
        let face = FaceExpectation {
            id,
            name,
            face_id: id,
            mana_cost,
            types: &["Instant"],
            keywords: &[] as &[Keyword],
            power_toughness: None,
        }
        .check();
        assert!(face.triggered_abilities.is_empty(), "{name}");
        assert!(face.activated_abilities.is_empty(), "{name}");
        assert!(face.modal_spell.is_none(), "{name}");
    }

    let taken = face("taken_by_nightmares");
    let [SpellEffectKind::Exile {
        subject: EffectSubject::Chosen(target),
    }, SpellEffectKind::Conditional {
        condition:
            GameCondition::BattlefieldAggregate {
                filter,
                aggregate: BattlefieldAggregate::Count,
                min: Some(1),
                max: None,
            },
        effect,
    }] = taken.spell_effect.as_slice()
    else {
        panic!("Taken by Nightmares has complete exile-then-conditional-Scry effects");
    };
    assert_eq!(target.kind, TargetKind::Creature);
    assert_eq!(filter.controllers, RelativePlayerSet::Controller);
    assert_eq!(filter.card_type, Some(CardTypeFilter::Enchantment));
    assert!(matches!(
        effect.as_ref(),
        SpellEffectKind::Scry {
            count: Amount::Fixed(2)
        }
    ));
    let targeting = taken.targeting.as_ref().expect("target creature");
    assert_eq!(targeting.groups.len(), 1);
    assert_eq!(targeting.groups[0].effect_indices, [0]);

    let failed = face("failed_fording");
    let [SpellEffectKind::ReturnToOwnersHand {
        subject: EffectSubject::Chosen(target),
    }, SpellEffectKind::Conditional {
        condition:
            GameCondition::BattlefieldAggregate {
                filter,
                aggregate: BattlefieldAggregate::Count,
                min: Some(1),
                max: None,
            },
        effect,
    }] = failed.spell_effect.as_slice()
    else {
        panic!("Failed Fording has complete return-then-conditional-Surveil effects");
    };
    assert_eq!(target.kind, TargetKind::AnyPermanent);
    assert_eq!(target.excluded_permanent_types, [PermanentTypeFilter::Land]);
    assert_eq!(filter.controllers, RelativePlayerSet::Controller);
    assert_eq!(filter.card_type, Some(CardTypeFilter::Land));
    assert_eq!(filter.required_subtypes, ["Desert"]);
    assert!(matches!(
        effect.as_ref(),
        SpellEffectKind::LibraryPartition {
            count: 1,
            top_min: 0,
            top_max: None,
            kind: LibraryPartitionKind::Surveil,
        }
    ));
    let targeting = failed.targeting.as_ref().expect("target nonland permanent");
    assert_eq!(targeting.groups.len(), 1);
    assert_eq!(targeting.groups[0].effect_indices, [0]);
}
