//! Printed identities for five reviewed Standard token and graveyard cards.

use tricerules_cards::{CardRegistry, Layout};

#[test]
fn issue_misc28_registers_five_exact_card_identities() {
    let registry = CardRegistry::global();
    for (id, name, mana, types, stats) in [
        (
            "eternal_student",
            "Eternal Student",
            "{3}{B}",
            &["Creature", "Zombie", "Warlock"][..],
            Some((4, 2)),
        ),
        (
            "envoy_of_okinec_ahau",
            "Envoy of Okinec Ahau",
            "{2}{W}",
            &["Creature", "Cat", "Advisor"][..],
            Some((3, 3)),
        ),
        (
            "tinkers_tote",
            "Tinker's Tote",
            "{2}{W}",
            &["Artifact"][..],
            None,
        ),
        (
            "broadcast_rambler",
            "Broadcast Rambler",
            "{4}{W}",
            &["Artifact", "Vehicle"][..],
            Some((5, 4)),
        ),
        (
            "suspicious_shambler",
            "Suspicious Shambler",
            "{3}{B}",
            &["Creature", "Zombie"][..],
            Some((4, 2)),
        ),
    ] {
        let card = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(card.layout, Layout::Normal, "{id}");
        let face = card.primary_face();
        assert_eq!(face.name, name, "{id}");
        assert_eq!(face.mana_cost.to_string(), mana, "{id}");
        assert_eq!(face.types, types, "{id}");
        assert_eq!(face.power.zip(face.toughness), stats, "{id}");
    }
}
