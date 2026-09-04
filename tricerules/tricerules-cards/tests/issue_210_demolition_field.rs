use tricerules_cards::primitives::{EffectSubject, PlayerRecipient, TargetController};
use tricerules_cards::{CardRegistry, PermanentTypeFilter, SearchDestination, SpellEffectKind};

const DEMOLITION_FIELD: &str = r#"(
  id: "demolition_field",
  name: "Demolition Field",
  face_id: "demolition_field",
  mana_cost: "",
  types: ["Land"],
  activated_abilities: [
    (
      ability_id: "activated_01",
      presentation: Fallback,
      costs: [Tap],
      effect: [ProduceMana(options: [(c: 1)])],
    ),
    (
      ability_id: "activated_02",
      presentation: Fallback,
      costs: [Mana("{2}"), Tap, SacrificeSelf],
      effect: [
        Destroy(subject: Chosen((
          kind: AnyPermanent,
          controller: Opponent,
          permanent_types: [Land],
          excluded_supertypes: ["Basic"],
        ))),
        SearchLibrary(
          who: ControllerOfTargetGroup(group_index: 0),
          optional: true,
          filter: Some((card_type: Some(BasicLand))),
          destination: Battlefield(tapped: false),
        ),
        SearchLibrary(
          who: Controller,
          optional: true,
          filter: Some((card_type: Some(BasicLand))),
          destination: Battlefield(tapped: false),
        ),
      ],
      targeting: Some((groups: [(
        min: 1,
        max: 1,
        prompt: "Choose target nonbasic land an opponent controls",
        effect_indices: [0],
      )])),
    ),
  ],
)"#;

#[test]
fn recipient_aware_optional_search_and_nonbasic_target_are_authorable() {
    let registry = CardRegistry::from_chunks_and_tokens(&[DEMOLITION_FIELD], &[])
        .expect("Demolition Field should use reusable search and target primitives");
    let ability = &registry
        .get("demolition_field")
        .expect("card")
        .primary_face()
        .activated_abilities[1];
    let [SpellEffectKind::Destroy {
        subject: EffectSubject::Chosen(target),
    }, first, second] = ability.effect.as_slice()
    else {
        panic!("ordered destroy and two searches")
    };
    assert_eq!(target.controller, TargetController::Opponent);
    assert_eq!(target.permanent_types, [PermanentTypeFilter::Land]);
    assert_eq!(target.excluded_supertypes, ["Basic"]);
    assert!(matches!(
        first,
        SpellEffectKind::SearchLibrary {
            who: PlayerRecipient::ControllerOfTargetGroup { group_index: 0 },
            optional: true,
            destination: SearchDestination::Battlefield { tapped: false },
            ..
        }
    ));
    assert!(matches!(
        second,
        SpellEffectKind::SearchLibrary {
            who: PlayerRecipient::Controller,
            optional: true,
            destination: SearchDestination::Battlefield { tapped: false },
            ..
        }
    ));
}

#[test]
fn supertype_target_predicates_reject_empty_duplicate_and_contradictory_values() {
    for filter in [
        "required_supertypes: [\"\"]",
        "excluded_supertypes: [\"Basic\", \"Basic\"]",
        "required_supertypes: [\"Legendary\"], excluded_supertypes: [\"Legendary\"]",
    ] {
        let card =
            DEMOLITION_FIELD.replace("excluded_supertypes: [\"Basic\"],", &format!("{filter},"));
        assert!(
            CardRegistry::from_chunks_and_tokens(&[&card], &[]).is_err(),
            "invalid filter unexpectedly loaded: {filter}"
        );
    }
}

#[test]
fn controller_of_target_group_search_requires_an_exact_permanent_group() {
    let unknown_group = DEMOLITION_FIELD.replace("group_index: 0", "group_index: 1");
    assert!(CardRegistry::from_chunks_and_tokens(&[&unknown_group], &[]).is_err());

    let optional_target = DEMOLITION_FIELD.replacen("min: 1,", "min: 0,", 1);
    assert!(CardRegistry::from_chunks_and_tokens(&[&optional_target], &[]).is_err());
}

#[test]
fn one_search_instruction_rejects_a_player_set_recipient() {
    let each_player = DEMOLITION_FIELD.replace("who: Controller,", "who: EachPlayer,");
    assert!(CardRegistry::from_chunks_and_tokens(&[&each_player], &[]).is_err());
}
