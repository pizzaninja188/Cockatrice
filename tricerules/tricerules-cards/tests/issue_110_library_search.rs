use tricerules_cards::primitives::{
    AbilityCost, Amount, CardSearchZone, CardTypeFilter, GraveyardDestination, LibraryBottomOrder,
    LibraryPlacement, PowerComparison, SearchDestination, SearchZoneSelection, SpellEffectKind,
    ZoneCardFilter,
};
use tricerules_cards::CardRegistry;

fn card(id: &str) -> &'static tricerules_cards::CardDefinition {
    CardRegistry::global()
        .get(id)
        .expect("issue #110 card is registered")
}

#[test]
fn issue_110_filters_cover_exact_name_recursive_type_or_and_printed_power() {
    let name = ZoneCardFilter {
        exact_name: Some("Tempest Hawk".into()),
        ..Default::default()
    };
    assert!(name.validate().is_ok());

    let creature_or_land = ZoneCardFilter {
        any_of: Some(vec![
            ZoneCardFilter {
                card_type: Some(CardTypeFilter::Creature),
                ..Default::default()
            },
            ZoneCardFilter {
                card_type: Some(CardTypeFilter::Land),
                ..Default::default()
            },
        ]),
        ..Default::default()
    };
    assert!(creature_or_land.validate().is_ok());

    let small_creature = ZoneCardFilter {
        card_type: Some(CardTypeFilter::Creature),
        printed_power: Some(PowerComparison::AtMost(2)),
        ..Default::default()
    };
    assert!(small_creature.validate().is_ok());
    assert!(ZoneCardFilter::default().validate().is_err());
}

#[test]
fn issue_110_search_and_look_cards_use_the_shared_primitives() {
    let tempest = card("tempest_hawk").primary_face();
    assert!(tempest.triggered_abilities.iter().any(|ability| matches!(
        ability.effect.as_slice(),
        [SpellEffectKind::ChooseResolutionBranch { branches, optional: true, .. }]
            if matches!(branches[0].effects.as_slice(),
                [SpellEffectKind::SearchLibrary {
                    filter: Some(ZoneCardFilter { exact_name: Some(name), .. }),
                    destination: SearchDestination::Hand,
                    shuffle: true,
                    reveal: true,
                    ..
                }] if name == "Tempest Hawk")
    )));

    let living = card("living_phone").primary_face();
    assert!(living.triggered_abilities.iter().any(|ability| matches!(
        ability.effect.as_slice(),
        [SpellEffectKind::LookChooseToHand {
            count: 5,
            filter: ZoneCardFilter {
                card_type: Some(CardTypeFilter::Creature),
                printed_power: Some(PowerComparison::AtMost(2)),
                ..
            },
            bottom_order: LibraryBottomOrder::Random,
        }]
    )));
}

#[test]
fn issue_110_say_its_name_and_altanak_are_complete() {
    let say = card("say_its_name");
    let [SpellEffectKind::Mill {
        count: Amount::Fixed(3),
        ..
    }, SpellEffectKind::ChooseGraveyardCard {
        filter: ZoneCardFilter {
            any_of: Some(filters),
            ..
        },
        destination: GraveyardDestination::Hand,
        optional: true,
        ..
    }] = say.primary_face().spell_effect.as_slice()
    else {
        panic!("Say Its Name mills then chooses from the current graveyard");
    };
    assert_eq!(filters.len(), 2);
    let ability = &say.primary_face().activated_abilities[0];
    assert!(matches!(
        ability.costs.as_slice(),
        [
            AbilityCost::ExileSelf,
            AbilityCost::ExileGraveyardCards {
                constraint: tricerules_cards::ObjectPaymentConstraint::ExactCount(2),
                exclude_source: true,
                ..
            }
        ]
    ));
    assert!(
        matches!(ability.effect.as_slice(), [SpellEffectKind::SearchLibrary {
        zones: SearchZoneSelection::PlayerChoice(zones),
        destination: SearchDestination::Battlefield { tapped: false },
        shuffle: true,
        ..
    }] if zones == &[CardSearchZone::Hand, CardSearchZone::Graveyard, CardSearchZone::Library])
    );

    let altanak = card("altanak,_the_thrice-called");
    assert!(matches!(
        altanak.primary_face().activated_abilities[0]
            .effect
            .as_slice(),
        [SpellEffectKind::MoveGraveyardCards {
            destination: GraveyardDestination::Battlefield { tapped: true },
            ..
        }]
    ));
}

#[test]
fn issue_110_conditional_and_owner_choice_cards_are_complete() {
    let embermouth = card("embermouth_sentinel").primary_face();
    assert!(
        matches!(embermouth.triggered_abilities[0].effect.as_slice(), [
            SpellEffectKind::ChooseResolutionBranch { branches, optional: true, .. }
        ] if matches!(branches[0].effects.as_slice(), [SpellEffectKind::SearchLibrary {
            destination: SearchDestination::TopOfLibrary,
            conditional_destination: Some(_),
            shuffle: true,
            reveal: true,
            ..
        }]))
    );

    let voyage = card("uncharted_voyage").primary_face();
    assert!(matches!(
        voyage.spell_effect.as_slice(),
        [
            SpellEffectKind::PutInOwnersLibrary {
                placement: LibraryPlacement::OwnerChoiceTopOrBottom,
                ..
            },
            SpellEffectKind::LibraryPartition {
                count: 1,
                top_min: 0,
                ..
            }
        ]
    ));

    let riverwalk = card("riverwalk_technique").primary_face();
    let modal = riverwalk
        .modal_spell
        .as_ref()
        .expect("Riverwalk Technique is modal");
    assert_eq!(
        (modal.min_modes, modal.max_modes, modal.modes.len()),
        (1, 1, 2)
    );
    assert!(matches!(
        modal.modes[0].effects.as_slice(),
        [SpellEffectKind::PutInOwnersLibrary {
            placement: LibraryPlacement::OwnerChoiceTopOrBottom,
            ..
        }]
    ));
    assert!(matches!(
        modal.modes[1].effects.as_slice(),
        [SpellEffectKind::CounterTargetSpell { .. }]
    ));
}
