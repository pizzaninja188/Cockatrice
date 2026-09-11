use tricerules_cards::primitives::{EntersTappedAffected, StaticAbilityDef};
use tricerules_cards::{AbilityCost, AbilityPresentation, CardRegistry};

fn mana_symbol(mana: &tricerules_cards::ManaAmount) -> char {
    match (mana.w, mana.u, mana.b, mana.r, mana.g, mana.c) {
        (1, 0, 0, 0, 0, 0) => 'W',
        (0, 1, 0, 0, 0, 0) => 'U',
        (0, 0, 1, 0, 0, 0) => 'B',
        (0, 0, 0, 1, 0, 0) => 'R',
        (0, 0, 0, 0, 1, 0) => 'G',
        _ => panic!("expected exactly one colored mana, got {mana:?}"),
    }
}

#[test]
fn issue_253_registers_all_twenty_four_lands_with_exact_ordered_options() {
    let registry = CardRegistry::global();
    for (id, expected_options) in [
        ("azorius_guildgate", "WU"),
        ("baron,_airship_kingdom", "UR"),
        ("boros_guildgate", "RW"),
        ("dimir_guildgate", "UB"),
        ("frontier_bivouac", "GUR"),
        ("gohn,_town_of_ruin", "BG"),
        ("golgari_guildgate", "BG"),
        ("gongaga,_reactor_town", "RG"),
        ("gruul_guildgate", "RG"),
        ("guadosalam,_farplane_gateway", "GU"),
        ("insomnia,_crown_city", "WB"),
        ("izzet_guildgate", "UR"),
        ("mystic_monastery", "URW"),
        ("nomad_outpost", "RWB"),
        ("opulent_palace", "BGU"),
        ("orzhov_guildgate", "WB"),
        ("rakdos_guildgate", "BR"),
        ("sandsteppe_citadel", "WBG"),
        ("selesnya_guildgate", "GW"),
        ("sharlayan,_nation_of_scholars", "WU"),
        ("simic_guildgate", "GU"),
        ("treno,_dark_city", "UB"),
        ("vector,_imperial_capital", "BR"),
        ("windurst,_federation_center", "GW"),
    ] {
        let face = registry
            .get(id)
            .unwrap_or_else(|| panic!("missing generated card {id}"))
            .primary_face();
        assert!(
            face.types.iter().any(|card_type| card_type == "Land"),
            "{id}"
        );

        let [entry] = face.static_abilities.as_slice() else {
            panic!("{id} must have one entry ability");
        };
        assert_eq!(entry.ability_id.as_str(), "static_01", "{id}");
        assert_eq!(
            entry.presentation,
            AbilityPresentation::OracleLines(vec![1]),
            "{id}"
        );
        assert_eq!(
            entry.definition,
            StaticAbilityDef::EntersTapped {
                affected: EntersTappedAffected::Self_,
                condition: None,
                unless_cost: None,
            },
            "{id}"
        );

        let [ability] = face.activated_abilities.as_slice() else {
            panic!("{id} must have one mana ability");
        };
        assert_eq!(ability.ability_id.as_str(), "activated_01", "{id}");
        assert_eq!(
            ability.presentation,
            AbilityPresentation::OracleLines(vec![2]),
            "{id}"
        );
        assert_eq!(ability.costs, [AbilityCost::Tap], "{id}");
        let actual_options = ability
            .mana_options()
            .expect("generated land must publish typed mana options")
            .iter()
            .map(mana_symbol)
            .collect::<String>();
        assert_eq!(actual_options, expected_options, "{id}");
    }
}
