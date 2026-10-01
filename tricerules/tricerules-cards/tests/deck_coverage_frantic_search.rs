use tricerules_cards::CardRegistry;

fn probe(effects: &str) -> Result<CardRegistry, String> {
    let draft = format!(
        r#"(id:"test",name:"Test",face_id:"test",mana_cost:"{{2}}{{U}}",
        types:["Instant"],spell_effect:[{effects}])"#
    );
    CardRegistry::from_chunks_and_tokens(&[&draft], &[]).map_err(|error| error.to_string())
}

const CHOICE: &str = "ChoosePermanents(chooser:Controller,filter:(kind:AnyPermanent,permanent_types:[Land]),min:0,max:3)";

#[test]
fn chosen_untap_requires_an_immediate_direct_permanent_choice() {
    probe(&format!("{CHOICE},UntapChosenPermanents")).unwrap();
    probe(&format!(
        "{},UntapChosenPermanents",
        CHOICE.replace("min:0,max:3", "min:1,max:1")
    ))
    .unwrap();
    for effects in [
        "UntapChosenPermanents".to_string(),
        "Scry(count:1),UntapChosenPermanents".to_string(),
        format!("{CHOICE},Scry(count:1),UntapChosenPermanents"),
        format!("{CHOICE},UntapChosenPermanents,UntapChosenPermanents"),
    ] {
        let error = probe(&effects).expect_err("incompatible receipt rejected");
        assert!(
            error.contains("immediately preceding direct ChoosePermanents"),
            "{error}"
        );
    }
    let wrapped =
        format!("Conditional(condition:ActivePlayer(players:Controller),effect:{CHOICE})");
    probe(&wrapped).unwrap();
    let error = probe(&format!("{wrapped},UntapChosenPermanents"))
        .expect_err("conditional producer rejected");
    assert!(error.contains("immediately preceding direct ChoosePermanents"));
    assert!(probe(&format!("{CHOICE},Conditional(condition:ActivePlayer(players:Controller),effect:UntapChosenPermanents)")).is_err());
}
