use tricerules_cards::primitives::{GraveyardFilter, ZoneCardFilter};
use tricerules_cards::CardRegistry;

#[test]
fn issue_198_zone_or_rejects_duplicate_terminal_predicates() {
    let filter: ZoneCardFilter = ron::from_str(
        "(any_of: Some([(card_type: Some(Creature)), (any_of: Some([(card_type: Some(Land)), (card_type: Some(Creature))]))]))"
    ).unwrap();
    assert!(
        filter.validate().is_err(),
        "nested duplicate leaves must fail registry validation"
    );
}

#[test]
fn issue_198_canonical_predicate_validation_and_roundtrip() {
    for source in [
        "(exact_name: Some(\"Tempest Hawk\"))",
        "(card_type: Some(Creature), required_subtypes: [\"Bird\"], printed_power: Some(AtMost(2)))",
        "(excluded_card_types: [Creature, Land], min_mana_value: Some(0), max_mana_value: Some(3))",
        "(required_subtypes: [\"Pirate\", \"Human\"], excluded_subtypes: [\"Zombie\"])",
        "(has_adventure: Some(false))",
        "(any_of: Some([(has_adventure: Some(true)), (any_of: Some([(card_type: Some(Land)), (exact_name: Some(\"Grizzly Bears\"))]))]))",
    ] {
        let filter: ZoneCardFilter = ron::from_str(source).unwrap();
        filter.validate().expect(source);
        let roundtrip: ZoneCardFilter = ron::from_str(&ron::to_string(&filter).unwrap()).unwrap();
        assert_eq!(filter, roundtrip);
        let card = format!(r#"(id: "test", name: "Test", face_id: "test", types: ["Sorcery"], spell_effect: [MoveGraveyardCards(filter: (owner: AnyPlayer, card: Some({source})), destination: Hand)])"#);
        CardRegistry::from_chunks_and_tokens(&[&card], &[]).expect("graveyard composes every predicate");
    }
    for source in [
        "()",
        "(exact_name: Some(\" \"))",
        "(required_subtypes: [\"\"])",
        "(excluded_subtypes: [\" \"])",
        "(required_subtypes: [\"Pirate\", \"Pirate\"])",
        "(excluded_subtypes: [\"Pirate\", \"Pirate\"])",
        "(required_subtypes: [\"Pirate\"], excluded_subtypes: [\"Pirate\"])",
        "(excluded_card_types: [Land, Land])",
        "(card_type: Some(Creature), excluded_card_types: [Creature])",
        "(min_mana_value: Some(4), max_mana_value: Some(3))",
        "(any_of: Some([]))",
        "(any_of: Some([(card_type: Some(Land))]))",
        "(any_of: Some([(), (card_type: Some(Land))]))",
    ] {
        let filter: ZoneCardFilter = ron::from_str(source).unwrap();
        assert!(filter.validate().is_err(), "{source}");
    }
    // Each leaf field must be rejected on an OR node, including explicit false/zero values.
    for leaf in [
        "exact_name: Some(\"Forest\")",
        "card_type: Some(Creature)",
        "excluded_card_types: [Land]",
        "min_mana_value: Some(0)",
        "max_mana_value: Some(0)",
        "required_subtypes: [\"Pirate\"]",
        "excluded_subtypes: [\"Pirate\"]",
        "has_adventure: Some(false)",
        "printed_power: Some(AtMost(0))",
    ] {
        let source = format!(
            "({leaf}, any_of: Some([(card_type: Some(Creature)), (card_type: Some(Land))]))"
        );
        let filter: ZoneCardFilter = ron::from_str(&source).unwrap();
        assert!(filter.validate().is_err(), "{source}");
    }
}

#[test]
fn issue_198_obsolete_fields_fail_closed_and_unrestricted_graveyards_remain_valid() {
    assert!(ron::from_str::<ZoneCardFilter>("(subtype: Some(\"Pirate\"))").is_err());
    for old in [
        "card_type: Some(Creature)",
        "excluded_card_types: [Land]",
        "min_mana_value: Some(1)",
        "max_mana_value: Some(3)",
        "required_subtypes: [\"Pirate\"]",
        "excluded_subtypes: [\"Pirate\"]",
        "has_adventure: Some(true)",
        "any_of: Some([(), ()])",
    ] {
        assert!(
            ron::from_str::<GraveyardFilter>(&format!("({old})")).is_err(),
            "{old}"
        );
    }
    for (context, valid) in [
        ("()", true),
        ("(owner: Opponent)", true),
        ("(owner: AnyPlayer, excluded_objects: [Source])", true),
        ("(card: Some(()))", false),
        ("(excluded_objects: [Source, Source])", false),
        ("(excluded_objects: [AttachedObject])", false),
    ] {
        let card = format!(
            r#"(id: "test", name: "Test", face_id: "test", types: ["Sorcery"], spell_effect: [MoveGraveyardCards(filter: {context}, destination: Hand)])"#
        );
        assert_eq!(
            CardRegistry::from_chunks_and_tokens(&[&card], &[]).is_ok(),
            valid,
            "{context}"
        );
    }
}

#[test]
fn issue_198_multiface_names_use_only_the_names_the_card_has_outside_the_stack() {
    // CR 709.4a/709.5: both split half/Room door names. CR 710.2, 712.8a, 715.4,
    // and 720.4: normal/front name for the other supported layouts.
    for (id, both_names) in [
        ("fire_ice", true),
        ("ticket_booth_tunnel_of_hate", true),
        ("bonecrusher_giant_stomp", false),
        ("reckless_waif_merciless_predator", false),
        ("cragcrown_pathway_timbercrown_pathway", false),
        ("akki_lavarunner_tok-tok,_volcano_born", false),
        ("sagu_wildling_roost_seek", false),
    ] {
        let definition = CardRegistry::global().get(id).expect(id);
        for (name, expected) in [
            (definition.face(0).unwrap().name.as_str(), true),
            (definition.face(1).unwrap().name.as_str(), both_names),
            (definition.name.as_str(), false),
        ] {
            let filter = ZoneCardFilter {
                exact_name: Some(name.into()),
                ..Default::default()
            };
            assert_eq!(
                definition.matches_zone_card_filter(&filter),
                expected,
                "{id}: {name}"
            );
        }
    }
}

#[test]
fn issue_198_printed_characteristics_preserve_multiface_and_changeling_semantics() {
    for (id, predicate, expected) in [
        ("fire_ice", "(min_mana_value: Some(4), max_mana_value: Some(4))", true),
        ("fire_ice", "(max_mana_value: Some(2))", false),
        ("ticket_booth_tunnel_of_hate", "(card_type: Some(Enchantment), required_subtypes: [\"Room\"], min_mana_value: Some(9), max_mana_value: Some(9))", true),
        ("bonecrusher_giant_stomp", "(card_type: Some(Creature), excluded_card_types: [Instant], required_subtypes: [\"Giant\"], has_adventure: Some(true), min_mana_value: Some(3), max_mana_value: Some(3), printed_power: Some(AtLeast(4)))", true),
        ("bonecrusher_giant_stomp", "(required_subtypes: [\"Adventure\"])", false),
        ("sagu_wildling_roost_seek", "(card_type: Some(Creature), excluded_card_types: [Sorcery], has_adventure: Some(false))", true),
        ("akki_lavarunner_tok-tok,_volcano_born", "(required_subtypes: [\"Goblin\", \"Warrior\"], printed_power: Some(AtMost(1)))", true),
        ("akki_lavarunner_tok-tok,_volcano_born", "(required_subtypes: [\"Shaman\"])", false),
        ("cragcrown_pathway_timbercrown_pathway", "(card_type: Some(Land), max_mana_value: Some(0))", true),
        ("reckless_waif_merciless_predator", "(required_subtypes: [\"Human\", \"Rogue\"], min_mana_value: Some(1), max_mana_value: Some(1), printed_power: Some(AtMost(1)))", true),
        ("reckless_waif_merciless_predator", "(printed_power: Some(AtLeast(3)))", false),
        ("endless_one", "(max_mana_value: Some(0), printed_power: Some(AtMost(0)))", true),
        ("changeling_wayfinder", "(required_subtypes: [\"Pirate\", \"Zombie\"])", true),
        ("changeling_wayfinder", "(excluded_subtypes: [\"Pirate\"])", false),
        ("changeling_wayfinder", "(required_subtypes: [\"Forest\"])", false),
        ("forest", "(printed_power: Some(AtMost(0)))", false),
    ] {
        let definition = CardRegistry::global().get(id).expect(id);
        let filter: ZoneCardFilter = ron::from_str(predicate).unwrap();
        filter.validate().unwrap();
        assert_eq!(definition.matches_zone_card_filter(&filter), expected, "{id}: {predicate}");
    }
}
