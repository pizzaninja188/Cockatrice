//! Printed identities for six pinned Standard combat spells.

use tricerules_cards::{CardRegistry, Layout};

#[test]
fn issue_misc39_registers_six_exact_card_identities() {
    let registry = CardRegistry::global();
    for (id, name, mana, types) in [
        (
            "impolite_entrance",
            "Impolite Entrance",
            "{R}",
            &["Sorcery"][..],
        ),
        (
            "youre_not_alone",
            "You're Not Alone",
            "{W}",
            &["Instant"][..],
        ),
        ("get_a_leg_up", "Get a Leg Up", "{G}", &["Instant"][..]),
        (
            "thoughtweft_charge",
            "Thoughtweft Charge",
            "{1}{G}",
            &["Instant"][..],
        ),
        (
            "dual-sun_technique",
            "Dual-Sun Technique",
            "{1}{W}",
            &["Instant"][..],
        ),
        ("thwip!", "Thwip!", "{W}", &["Instant"][..]),
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
