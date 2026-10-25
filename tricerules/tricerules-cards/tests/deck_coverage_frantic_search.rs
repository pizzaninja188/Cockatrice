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
fn flat_chosen_untap_requires_a_typed_program_input() {
    for effects in [
        "UntapChosenPermanents".to_string(),
        format!("{CHOICE},UntapChosenPermanents"),
        format!(
            "{},UntapChosenPermanents",
            CHOICE.replace("min:0,max:3", "min:1,max:1")
        ),
        "Scry(count:1),UntapChosenPermanents".to_string(),
        format!("{CHOICE},Scry(count:1),UntapChosenPermanents"),
        format!("{CHOICE},UntapChosenPermanents,UntapChosenPermanents"),
    ] {
        let error = probe(&effects).expect_err("incompatible receipt rejected");
        assert!(
            error.contains("UntapChosenPermanents requires an authored Program input"),
            "{error}"
        );
    }
    let wrapped =
        format!("Conditional(condition:ActivePlayer(players:Controller),effect:{CHOICE})");
    probe(&wrapped).unwrap();
    let error = probe(&format!("{wrapped},UntapChosenPermanents"))
        .expect_err("flat lists cannot bind an explicit Program input");
    assert!(error.contains("UntapChosenPermanents requires an authored Program input"));
    let nested = probe(&format!(
        "{CHOICE},Conditional(condition:ActivePlayer(players:Controller),effect:UntapChosenPermanents)"
    ))
    .expect_err("nested flat consumers also require a Program input");
    assert!(
        nested.contains("requires an authored Program input")
            || nested.contains("Conditional currently supports")
    );
}
