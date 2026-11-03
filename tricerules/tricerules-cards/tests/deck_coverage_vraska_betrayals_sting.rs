//! Exact complete definition of Vraska, Betrayal's Sting (Oracle 05d09c55-f551-4cb4-a8d2-4811dccc848d).
use tricerules_cards::primitives::{
    AbilityCost, Amount, CounterKind, EffectSubject, LifeAmount, PermanentTypeFilter,
    PlayerRecipient, ResolvingEffectDuration, ResolvingPermanentModifier, SpellEffectKind,
    StaticAbilityDef, TargetKind, TypeLineArtifactSubtype,
};
use tricerules_cards::AbilityPresentation;

#[test]
fn vraska_exact_identity_and_all_three_loyalty_abilities() {
    const ID: &str = "vraska,_betrayals_sting";
    let registry = tricerules_cards::registry::global();
    let card = registry
        .get(ID)
        .expect("complete Vraska must be registered");
    assert_eq!(registry.id_for_name("Vraska, Betrayal's Sting"), Some(ID));
    assert!(!card.commander_setup_only);
    assert_eq!(card.face_count(), 1);
    let face = card.primary_face();
    assert_eq!(face.face_id.as_str(), "vraska_betrayals_sting");
    assert_eq!(face.mana_cost.to_string(), "{4}{B}{B/P}");
    assert_eq!(face.mana_cost.mana_value(), 6);
    assert_eq!(face.supertypes, ["Legendary"]);
    assert_eq!(face.types, ["Planeswalker", "Vraska"]);
    assert_eq!(face.loyalty, Some(6));
    assert_eq!(face.static_abilities.len(), 2);
    assert!(face
        .static_abilities
        .iter()
        .any(|ability| ability.definition == StaticAbilityDef::Compleated
            && ability.presentation == AbilityPresentation::OracleLines(vec![1])));
    assert!(face.static_abilities.iter().any(|ability| matches!(
        ability.definition,
        StaticAbilityDef::EntersWithCounters {
            counter: CounterKind::Loyalty,
            ..
        }
    )));
    assert_eq!(face.activated_abilities.len(), 3);
    for (index, (loyalty, line)) in [(0, 2), (-2, 3), (-9, 4)].into_iter().enumerate() {
        let ability = &face.activated_abilities[index];
        assert_eq!(
            ability.ability_id.as_str(),
            format!("activated_{:02}", index + 1)
        );
        assert_eq!(ability.costs, [AbilityCost::Loyalty(loyalty)]);
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![line])
        );
        assert!(ability.program.is_none());
        assert!(ability.modal.is_none());
    }
    let zero = &face.activated_abilities[0];
    assert!(zero.targeting.is_none());
    assert!(matches!(
        zero.effect.as_slice(),
        [
            SpellEffectKind::Draw {
                count: Amount::Fixed(1),
                who: PlayerRecipient::Controller
            },
            SpellEffectKind::LoseLife {
                amount: LifeAmount::Fixed(1),
                who: PlayerRecipient::Controller
            },
            SpellEffectKind::Proliferate
        ]
    ));
    let treasure = &face.activated_abilities[1];
    let groups = &treasure.targeting.as_ref().unwrap().groups;
    assert_eq!(groups.len(), 1);
    assert_eq!((groups[0].min, groups[0].max), (1, 1));
    assert_eq!(groups[0].effect_indices, [0, 1, 2]);
    let [SpellEffectKind::ApplyPermanentModifier {
        subject: EffectSubject::Chosen(target),
        modifier: ResolvingPermanentModifier::SetTypeLine(types),
        duration: ResolvingEffectDuration::Indefinite,
    }, SpellEffectKind::RemoveAllAbilities {
        subject: removal,
        duration: ResolvingEffectDuration::Indefinite,
    }, SpellEffectKind::ApplyPermanentModifier {
        subject: grant_subject,
        modifier: ResolvingPermanentModifier::GrantActivatedAbility(grant),
        duration: ResolvingEffectDuration::Indefinite,
    }] = treasure.effect.as_slice()
    else {
        panic!("exact same-object type/removal/grant instruction order");
    };
    assert_eq!(target.kind, TargetKind::Creature);
    assert_eq!(removal, &EffectSubject::Chosen(target.clone()));
    assert_eq!(grant_subject, removal);
    assert_eq!(types.card_types, [PermanentTypeFilter::Artifact]);
    assert_eq!(types.artifact_types, [TypeLineArtifactSubtype::Treasure]);
    assert!(types.creature_types.is_empty() && types.land_types.is_empty());
    assert_eq!(grant.ability_id.as_str(), "treasure_mana");
    assert_eq!(grant.costs, [AbilityCost::Tap, AbilityCost::SacrificeSelf]);
    let [SpellEffectKind::ProduceMana { options, .. }] = grant.effect.as_slice() else {
        panic!("Treasure mana");
    };
    assert_eq!(options.len(), 5);
    assert_eq!(
        options
            .iter()
            .map(|mana| (mana.w, mana.u, mana.b, mana.r, mana.g, mana.c))
            .collect::<Vec<_>>(),
        [
            (1, 0, 0, 0, 0, 0),
            (0, 1, 0, 0, 0, 0),
            (0, 0, 1, 0, 0, 0),
            (0, 0, 0, 1, 0, 0),
            (0, 0, 0, 0, 1, 0)
        ]
    );
    let ultimate = &face.activated_abilities[2];
    assert!(
        matches!(ultimate.effect.as_slice(), [SpellEffectKind::PoisonTargetUpToNine { target }] if target.kind == TargetKind::AnyPlayer)
    );
    let groups = &ultimate.targeting.as_ref().unwrap().groups;
    assert_eq!(groups.len(), 1);
    assert_eq!((groups[0].min, groups[0].max), (1, 1));
    assert_eq!(groups[0].effect_indices, [0]);
}
