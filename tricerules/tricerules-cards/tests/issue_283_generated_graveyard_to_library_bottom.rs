use tricerules_cards::primitives::{
    GraveyardDestination, GraveyardOwner, TargetRole, TargetSchema,
};
use tricerules_cards::{
    AbilityCost, AbilityPresentation, CardRegistry, CharacteristicDefiningAbility, ManaCost,
    SpellEffectKind,
};

fn assert_graveyard_to_bottom_ability(
    ability: &tricerules_cards::ActivatedAbilityDef,
    oracle_line: u16,
) {
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![oracle_line])
    );
    assert_eq!(
        ability.source_zone,
        tricerules_cards::AbilitySourceZone::Battlefield
    );
    assert_eq!(
        ability.costs,
        [AbilityCost::Mana(ManaCost::parse("{2}").unwrap())]
    );
    assert!(ability.cost_modifiers.is_empty());
    assert_eq!(ability.timing, tricerules_cards::ActivationTiming::Normal);
    assert!(ability.conditions.is_empty());
    assert!(ability.activation_limit.is_none());

    let [SpellEffectKind::MoveGraveyardCards {
        filter,
        destination,
        linked_exile_id,
    }] = ability.effect.as_slice()
    else {
        panic!("expected one controller-graveyard-to-library-bottom effect");
    };
    assert_eq!(filter.owner, GraveyardOwner::Controller);
    assert!(filter.excluded_objects.is_empty());
    assert!(filter.card.is_none());
    assert_eq!(*destination, GraveyardDestination::LibraryBottom);
    assert!(linked_exile_id.is_none());

    let schema = TargetSchema::compile(&ability.effect, ability.targeting.as_ref())
        .expect("generated issue #283 target schema");
    let [group] = schema.groups.as_slice() else {
        panic!("expected one mandatory graveyard target group");
    };
    assert_eq!((group.min, group.max), (1, 1));
    assert_eq!(group.prompt, "Choose target card from your graveyard");
    let [binding] = group.bindings.as_slice() else {
        panic!("expected one graveyard target binding");
    };
    assert!(matches!(
        binding.role,
        TargetRole::GraveyardCard(filter)
            if filter.owner == GraveyardOwner::Controller && filter.card.is_none()
    ));
}

#[test]
fn issue_283_registers_barkform_harvester_and_tomb_trawler() {
    let registry = CardRegistry::global();

    let barkform = registry
        .get("barkform_harvester")
        .expect("Barkform Harvester should be generated")
        .primary_face();
    assert_eq!(barkform.name, "Barkform Harvester");
    assert_eq!(barkform.face_id.as_str(), "barkform_harvester");
    assert_eq!(barkform.mana_cost.to_string(), "{3}");
    assert_eq!(barkform.types, ["Artifact", "Creature", "Shapeshifter"]);
    assert_eq!((barkform.power, barkform.toughness), (Some(2), Some(3)));
    assert_eq!(barkform.keywords, [tricerules_cards::Keyword::Reach]);
    let [changeling] = barkform.characteristic_defining_abilities.as_slice() else {
        panic!("Barkform Harvester must preserve its Changeling ability");
    };
    assert_eq!(
        changeling.definition,
        CharacteristicDefiningAbility::Changeling
    );
    assert_eq!(
        changeling.presentation,
        AbilityPresentation::OracleLines(vec![1])
    );
    let [barkform_ability] = barkform.activated_abilities.as_slice() else {
        panic!("Barkform Harvester should have exactly one activated ability");
    };
    assert_eq!(barkform_ability.ability_id.as_str(), "activated_01");
    assert_graveyard_to_bottom_ability(barkform_ability, 3);

    let tomb = registry
        .get("tomb_trawler")
        .expect("Tomb Trawler should be generated")
        .primary_face();
    assert_eq!(tomb.name, "Tomb Trawler");
    assert_eq!(tomb.face_id.as_str(), "tomb_trawler");
    assert_eq!(tomb.mana_cost.to_string(), "{2}");
    assert_eq!(tomb.types, ["Artifact", "Creature", "Golem"]);
    assert_eq!((tomb.power, tomb.toughness), (Some(0), Some(4)));
    assert!(tomb.keywords.is_empty());
    assert!(tomb.characteristic_defining_abilities.is_empty());
    let [tomb_ability] = tomb.activated_abilities.as_slice() else {
        panic!("Tomb Trawler should have exactly one activated ability");
    };
    assert_eq!(tomb_ability.ability_id.as_str(), "activated_01");
    assert_graveyard_to_bottom_ability(tomb_ability, 1);
}

#[test]
fn issue_283_matches_existing_authored_library_bottom_shape() {
    let registry = CardRegistry::global();
    let malevolent = registry
        .get("malevolent_chandelier")
        .expect("Malevolent Chandelier is the authored library-bottom reference")
        .primary_face();
    let [ability] = malevolent.activated_abilities.as_slice() else {
        panic!("Malevolent Chandelier should have one authored activated ability");
    };
    assert_eq!(
        ability.presentation,
        AbilityPresentation::OracleLines(vec![2])
    );
    assert_eq!(
        ability.source_zone,
        tricerules_cards::AbilitySourceZone::Battlefield
    );
    assert_eq!(
        ability.costs,
        [AbilityCost::Mana(ManaCost::parse("{2}").unwrap())]
    );
    assert!(ability.targeting.is_none());
    assert_eq!(
        ability.timing,
        tricerules_cards::ActivationTiming::SorcerySpeed
    );
    let [SpellEffectKind::MoveGraveyardCards {
        filter,
        destination,
        linked_exile_id,
    }] = ability.effect.as_slice()
    else {
        panic!("Malevolent Chandelier should use the typed graveyard movement effect");
    };
    assert_eq!(filter.owner, GraveyardOwner::AnyPlayer);
    assert_eq!(*destination, GraveyardDestination::LibraryBottom);
    assert!(linked_exile_id.is_none());
}
