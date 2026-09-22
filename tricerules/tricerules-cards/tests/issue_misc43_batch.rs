//! Printed identities for five pinned Standard spells.

use tricerules_cards::{CardRegistry, Layout};

#[test]
fn issue_misc43_registers_five_exact_card_identities() {
    let registry = CardRegistry::global();
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
}
