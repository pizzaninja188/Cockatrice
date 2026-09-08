use tricerules_cards::primitives::{EffectSubject, SpellEffectKind, TargetOwner};
use tricerules_cards::{AbilityPresentation, CardRegistry};

#[test]
fn issue_231_all_three_cards_are_complete_with_stable_modal_presentation() {
    let registry = CardRegistry::global();
    for (id, name, mana) in [
        ("annul", "Annul", "{U}"),
        ("flashfreeze", "Flashfreeze", "{1}{U}"),
        ("get_out", "Get Out", "{U}{U}"),
    ] {
        let card = registry.get(id).unwrap_or_else(|| panic!("missing {name}"));
        assert_eq!(registry.id_for_name(name), Some(id));
        assert_eq!(card.primary_face().mana_cost.to_string(), mana);
        assert_eq!(card.primary_face().types, ["Instant"]);
    }
    let face = registry.get("get_out").unwrap().primary_face();
    let modal = face.modal_spell.as_ref().unwrap();
    assert_eq!((modal.min_modes, modal.max_modes), (1, 1));
    assert_eq!(modal.modes.len(), 2);
    for (index, mode) in modal.modes.iter().enumerate() {
        assert_eq!(
            mode.presentation,
            AbilityPresentation::OracleLines(vec![index as u16 + 2])
        );
    }
    let [SpellEffectKind::ReturnToOwnersHand {
        subject: EffectSubject::Chosen(filter),
    }] = modal.modes[1].effects.as_slice()
    else {
        panic!("Get Out returns its chosen permanents")
    };
    assert_eq!(filter.owner, TargetOwner::You);
    let group = &modal.modes[1].targeting.as_ref().unwrap().groups[0];
    assert_eq!((group.min, group.max), (1, 2));
}
