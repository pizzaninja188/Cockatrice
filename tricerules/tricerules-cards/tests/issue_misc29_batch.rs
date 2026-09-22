//! Printed identities for five reviewed Standard artifact-token cards.

use tricerules_cards::{CardRegistry, Layout};

#[test]
fn issue_misc29_registers_five_exact_card_identities() {
    let registry = CardRegistry::global();
    for (id, name, mana, types, stats) in [
        (
            "biomechan_engineer",
            "Biomechan Engineer",
            "{G}{U}",
            &["Creature", "Insect", "Artificer"][..],
            Some((2, 2)),
        ),
        (
            "sweettooth_witch",
            "Sweettooth Witch",
            "{2}{B}",
            &["Creature", "Human", "Warlock"][..],
            Some((3, 2)),
        ),
        (
            "redrock_sentinel",
            "Redrock Sentinel",
            "{3}",
            &["Artifact", "Creature", "Golem"][..],
            Some((2, 4)),
        ),
        (
            "flick_a_coin",
            "Flick a Coin",
            "{2}{R}",
            &["Instant"][..],
            None,
        ),
        (
            "brasss_bounty",
            "Brass's Bounty",
            "{6}{R}",
            &["Sorcery"][..],
            None,
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
