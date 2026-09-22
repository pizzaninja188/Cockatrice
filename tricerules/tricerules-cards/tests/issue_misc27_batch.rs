//! Printed identities and token characteristics for five pinned Standard entry triggers.

use tricerules_cards::{CardRegistry, Color, Keyword, Layout};

#[test]
fn issue_misc27_registers_five_exact_card_identities() {
    let registry = CardRegistry::global();
    for (id, name, mana, types, stats, keywords) in [
        (
            "nimble_thopterist",
            "Nimble Thopterist",
            "{3}{U}",
            &["Creature", "Vedalken", "Artificer"][..],
            (3, 2),
            &[][..],
        ),
        (
            "eager_glyphmage",
            "Eager Glyphmage",
            "{3}{W}",
            &["Creature", "Cat", "Cleric"][..],
            (3, 3),
            &[][..],
        ),
        (
            "mechanized_ninja_cavalry",
            "Mechanized Ninja Cavalry",
            "{1}{R/W}",
            &["Artifact", "Creature", "Robot", "Ninja"][..],
            (1, 1),
            &[][..],
        ),
        (
            "oltec_cloud_guard",
            "Oltec Cloud Guard",
            "{3}{W}",
            &["Creature", "Human", "Soldier"][..],
            (3, 2),
            &[Keyword::Flying][..],
        ),
        (
            "guarded_heir",
            "Guarded Heir",
            "{5}{W}",
            &["Creature", "Human", "Noble"][..],
            (1, 1),
            &[Keyword::Lifelink][..],
        ),
    ] {
        let card = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(card.layout, Layout::Normal, "{id}");
        let face = card.primary_face();
        assert_eq!(face.name, name, "{id}");
        assert_eq!(face.mana_cost.to_string(), mana, "{id}");
        assert_eq!(face.types, types, "{id}");
        assert_eq!(face.power.zip(face.toughness), Some(stats), "{id}");
        assert_eq!(face.keywords, keywords, "{id}");
    }
}

#[test]
fn issue_misc27_token_templates_match_printed_characteristics() {
    let registry = CardRegistry::global();
    for (id, name, types, stats, colors, keywords) in [
        (
            "thopter_c_1_1_flying",
            "Thopter",
            &["Artifact", "Creature", "Thopter"][..],
            (1, 1),
            Some(vec![]),
            &[Keyword::Flying][..],
        ),
        (
            "inkling_wb_1_1_flying",
            "Inkling",
            &["Creature", "Inkling"][..],
            (1, 1),
            Some(vec![Color::White, Color::Black]),
            &[Keyword::Flying][..],
        ),
        (
            "robot_c_1_1",
            "Robot",
            &["Artifact", "Creature", "Robot"][..],
            (1, 1),
            Some(vec![]),
            &[][..],
        ),
        (
            "gnome_c_1_1",
            "Gnome",
            &["Artifact", "Creature", "Gnome"][..],
            (1, 1),
            Some(vec![]),
            &[][..],
        ),
        (
            "knight_w_3_3",
            "Knight",
            &["Creature", "Knight"][..],
            (3, 3),
            Some(vec![Color::White]),
            &[][..],
        ),
    ] {
        let card = registry
            .get(id)
            .unwrap_or_else(|| panic!("missing token {id}"));
        assert!(registry.is_token(id), "{id}");
        let face = card.primary_face();
        assert_eq!(face.name, name, "{id}");
        assert_eq!(face.types, types, "{id}");
        assert_eq!(face.power.zip(face.toughness), Some(stats), "{id}");
        assert_eq!(face.colors_override, colors, "{id}");
        assert_eq!(face.keywords, keywords, "{id}");
    }
}
