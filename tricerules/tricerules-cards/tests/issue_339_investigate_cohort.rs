//! Printed identities for the #339 investigate cohorts.

use tricerules_cards::Layout;

#[test]
fn issue_339_cohort_registers_exact_card_identities() {
    let registry = tricerules_cards::registry::global();
    for (id, name, mana, supertypes, types, body) in [
        (
            "the_chase_is_on",
            "The Chase Is On",
            "{2}{R}",
            &[][..],
            &["Instant"][..],
            None,
        ),
        (
            "toxin_analysis",
            "Toxin Analysis",
            "{B}",
            &[][..],
            &["Instant"][..],
            None,
        ),
        (
            "they_went_this_way",
            "They Went This Way",
            "{2}{G}",
            &[][..],
            &["Sorcery"][..],
            None,
        ),
        (
            "magnifying_glass",
            "Magnifying Glass",
            "{3}",
            &[][..],
            &["Artifact"][..],
            None,
        ),
        (
            "cold_case_cracker",
            "Cold Case Cracker",
            "{3}{U}",
            &[][..],
            &["Creature", "Spirit", "Detective"][..],
            Some((3, 3)),
        ),
        (
            "ezrim,_agency_chief",
            "Ezrim, Agency Chief",
            "{1}{W}{W}{U}{U}",
            &["Legendary"][..],
            &["Creature", "Archon", "Detective"][..],
            Some((5, 5)),
        ),
        (
            "detectives_satchel",
            "Detective's Satchel",
            "{2}{U}{R}",
            &[][..],
            &["Artifact"][..],
            None,
        ),
        (
            "alquist_proft,_master_sleuth",
            "Alquist Proft, Master Sleuth",
            "{1}{W}{U}",
            &["Legendary"][..],
            &["Creature", "Human", "Detective"][..],
            Some((3, 3)),
        ),
        (
            "out_cold",
            "Out Cold",
            "{3}{U}",
            &[][..],
            &["Instant"][..],
            None,
        ),
        (
            "homicide_investigator",
            "Homicide Investigator",
            "{1}{B}",
            &[][..],
            &["Creature", "Human", "Detective"][..],
            Some((2, 2)),
        ),
        (
            "hostile_investigator",
            "Hostile Investigator",
            "{3}{B}",
            &[][..],
            &["Creature", "Ogre", "Rogue", "Detective"][..],
            Some((4, 3)),
        ),
        (
            "malcolm,_the_eyes",
            "Malcolm, the Eyes",
            "{U}{R}",
            &["Legendary"][..],
            &["Creature", "Siren", "Pirate"][..],
            Some((2, 2)),
        ),
        (
            "curious_inquiry",
            "Curious Inquiry",
            "{U}",
            &[][..],
            &["Enchantment", "Aura"][..],
            None,
        ),
        (
            "agent_13,_sharon_carter",
            "Agent 13, Sharon Carter",
            "{2}{W}",
            &["Legendary"][..],
            &["Creature", "Human", "Spy", "Hero"][..],
            Some((3, 2)),
        ),
        (
            "meddling_youths",
            "Meddling Youths",
            "{3}{R}{W}",
            &[][..],
            &["Creature", "Human", "Detective"][..],
            Some((4, 5)),
        ),
        (
            "chalk_outline",
            "Chalk Outline",
            "{3}{G}",
            &[][..],
            &["Enchantment"][..],
            None,
        ),
        (
            "drag_the_canal",
            "Drag the Canal",
            "{U}{B}",
            &[][..],
            &["Instant"][..],
            None,
        ),
    ] {
        let card = registry.get(id).unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(card.layout, Layout::Normal, "{id}");
        let face = card.primary_face();
        assert_eq!(face.name, name, "{id}");
        assert_eq!(face.mana_cost.to_string(), mana, "{id}");
        assert_eq!(face.supertypes, supertypes, "{id}");
        assert_eq!(face.types, types, "{id}");
        assert_eq!(face.power.zip(face.toughness), body, "{id}");
    }
}
