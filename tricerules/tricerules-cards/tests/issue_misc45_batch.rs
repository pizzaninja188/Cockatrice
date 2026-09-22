//! Printed identities for five pinned Standard spells.

use tricerules_cards::{CardRegistry, Layout};

#[test]
fn issue_misc45_registers_five_exact_card_identities() {
    let registry = CardRegistry::global();
    for (id, name, mana, types) in [
        (
            "roadside_blowout",
            "Roadside Blowout",
            "{2}{U}",
            &["Sorcery"][..],
        ),
        (
            "map_the_frontier",
            "Map the Frontier",
            "{3}{G}",
            &["Sorcery"][..],
        ),
        (
            "circuitous_route",
            "Circuitous Route",
            "{3}{G}",
            &["Sorcery"][..],
        ),
        (
            "grow_extra_arms",
            "Grow Extra Arms",
            "{1}{G}",
            &["Instant"][..],
        ),
        (
            "auspicious_arrival",
            "Auspicious Arrival",
            "{1}{W}",
            &["Instant"][..],
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
