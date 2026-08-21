use tricerules_cards::primitives::{
    CardTypeFilter, EffectSubject, SearchDestination, SpellEffectKind, TargetController, TargetKind,
};
use tricerules_cards::CardRegistry;

fn assert_fight_subjects(first: &EffectSubject, second: &EffectSubject) {
    assert!(matches!(
        first,
        EffectSubject::Chosen(filter)
            if filter.kind == TargetKind::Creature
                && filter.controller == TargetController::You
    ));
    assert!(matches!(
        second,
        EffectSubject::Chosen(filter)
            if filter.kind == TargetKind::Creature
                && filter.controller == TargetController::NotYou
    ));
}

#[test]
fn issue_117_prey_upon_uses_two_distinct_chosen_fighters() {
    let definition = CardRegistry::global()
        .get("prey_upon")
        .expect("Prey Upon is registered");
    assert!(definition.partial.is_none());
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{G}");
    assert_eq!(face.types, ["Sorcery"]);
    let [SpellEffectKind::Fight { first, second }] = face.spell_effect.as_slice() else {
        panic!("Prey Upon uses the shared Fight primitive");
    };
    assert_fight_subjects(first, second);

    let groups = &face.targeting.as_ref().expect("grouped targeting").groups;
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[0].effect_indices, [0]);
    assert_eq!(groups[1].effect_indices, [0]);
    assert_eq!(groups[1].distinct_from, [0]);
}

#[test]
fn issue_117_bushwhack_has_search_and_two_target_fight_modes() {
    let definition = CardRegistry::global()
        .get("bushwhack")
        .expect("Bushwhack is registered");
    assert!(definition.partial.is_none());
    let face = definition.primary_face();
    assert_eq!(face.mana_cost.to_string(), "{G}");
    assert_eq!(face.types, ["Sorcery"]);

    let modal = face.modal_spell.as_ref().expect("Bushwhack is modal");
    assert_eq!((modal.min_modes, modal.max_modes), (1, 1));
    assert!(matches!(
        modal.modes[0].effects.as_slice(),
        [SpellEffectKind::SearchLibrary {
            filter: Some(tricerules_cards::primitives::LibraryCardFilter {
                card_type: Some(CardTypeFilter::BasicLand),
                subtype: None,
            }),
            destination: SearchDestination::Hand,
            shuffle: true,
            reveal: true,
        }]
    ));
    let [SpellEffectKind::Fight { first, second }] = modal.modes[1].effects.as_slice() else {
        panic!("Bushwhack's second mode uses Fight");
    };
    assert_fight_subjects(first, second);

    let groups = &modal.modes[1]
        .targeting
        .as_ref()
        .expect("fight mode grouped targeting")
        .groups;
    assert_eq!(groups.len(), 2);
    assert_eq!(groups[1].distinct_from, [0]);
}
