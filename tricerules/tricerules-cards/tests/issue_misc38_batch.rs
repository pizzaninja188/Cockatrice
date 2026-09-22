//! Printed identities for five pinned Standard combat spells.

use tricerules_cards::{CardRegistry, Layout};

#[test]
fn issue_misc38_registers_five_exact_card_identities() {
    let registry = CardRegistry::global();
    for (id, name, mana, types) in [
        ("crash_through", "Crash Through", "{R}", &["Sorcery"][..]),
        ("efflorescence", "Efflorescence", "{2}{G}", &["Instant"][..]),
        (
            "might_of_the_meek",
            "Might of the Meek",
            "{R}",
            &["Instant"][..],
        ),
        (
            "pedal_to_the_metal",
            "Pedal to the Metal",
            "{X}{R}",
            &["Instant"][..],
        ),
        ("take_the_fall", "Take the Fall", "{U}", &["Instant"][..]),
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
