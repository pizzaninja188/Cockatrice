//! Printed identities for five pinned Standard spells.

use tricerules_cards::Layout;

#[test]
fn issue_misc43_registers_five_exact_card_identities() {
    let registry = tricerules_cards::registry::global();
    for (id, name, mana, types) in [
        ("deadly_plot", "Deadly Plot", "{3}{B}", &["Instant"][..]),
        (
            "break_down_the_door",
            "Break Down the Door",
            "{2}{G}",
            &["Instant"][..],
        ),
        (
            "spectral_interference",
            "Spectral Interference",
            "{1}{U}",
            &["Instant"][..],
        ),
        (
            "calamitous_tide",
            "Calamitous Tide",
            "{4}{U}{U}",
            &["Sorcery"][..],
        ),
        (
            "ozais_cruelty",
            "Ozai's Cruelty",
            "{2}{B}",
            &["Sorcery", "Lesson"][..],
        ),
    ] {
        let card = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(card.layout, Layout::Normal, "{id}");
        let face = card.primary_face();
        assert_eq!(face.name, name, "{id}");
        assert_eq!(face.mana_cost.to_string(), mana, "{id}");
        assert_eq!(face.types, types, "{id}");
        assert!(face.power.is_none() && face.toughness.is_none(), "{id}");
    }

    let face = registry.get("calamitous_tide").unwrap().primary_face();
    assert!(face.spell_effect.is_empty());
    let program = face.spell_program.as_ref().expect("authored spell program");
    assert_eq!(program.target_bindings.len(), 1);
    assert_eq!(program.target_bindings[0].instruction_id, "return_creature");
    assert_eq!(
        program.target_bindings[0].role,
        tricerules_cards::ProgramTargetRole::Subject
    );
    assert_eq!(program.target_bindings[0].target_group_index, 0);
    let [group] = face.targeting.as_ref().unwrap().groups.as_slice() else {
        panic!("Calamitous Tide has one optional target group");
    };
    assert_eq!((group.min, group.max), (0, 2));
    assert!(group.effect_indices.is_empty());
}
