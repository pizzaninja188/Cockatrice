use tricerules_cards::Color;

#[test]
fn four_deck_commanders_resolve_with_source_backed_identity() {
    let registry = tricerules_cards::registry::global();
    for (id, name, identity, setup_only) in [
        (
            "atraxa,_praetors_voice",
            "Atraxa, Praetors' Voice",
            vec![Color::White, Color::Blue, Color::Black, Color::Green],
            false,
        ),
        (
            "kami_of_the_crescent_moon",
            "Kami of the Crescent Moon",
            vec![Color::Blue],
            false,
        ),
        (
            "bello,_bard_of_the_brambles",
            "Bello, Bard of the Brambles",
            vec![Color::Red, Color::Green],
            true,
        ),
        (
            "daretti,_scrap_savant",
            "Daretti, Scrap Savant",
            vec![Color::Red],
            true,
        ),
    ] {
        let card = registry
            .get(id)
            .expect("commander setup card is registered");
        assert_eq!(card.name, name);
        assert_eq!(card.color_identity(), identity);
        assert_eq!(card.commander_setup_only, setup_only);
    }
}
