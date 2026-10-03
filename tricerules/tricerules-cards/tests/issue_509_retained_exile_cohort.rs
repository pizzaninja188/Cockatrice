use tricerules_cards::registry::RegistryError;
use tricerules_cards::CardRegistry;

const CAPTURE: &str = r#"ExileGraveyards(players: All, filter: Some((card_type: Some(Artifact))), capture_exile_cohort: Some("original_graveyards"))"#;
const RETURN: &str = r#"ReturnExiledCohortToOwnersBattlefield(cohort_id: "original_graveyards")"#;

fn invalid(ron: &str, fragment: &str) {
    let result = CardRegistry::from_chunks_and_tokens(&[ron], &[]);
    assert!(
        matches!(result, Err(RegistryError::InvalidCard { ref reason, .. }) if reason.contains(fragment)),
        "expected {fragment}: {result:?}"
    );
}

fn spell(effects: &str) -> String {
    format!(
        r#"(id: "cohort_probe", name: "Cohort Probe", face_id: "cohort_probe",
        mana_cost: "{{3}}{{R}}{{R}}", types: ["Sorcery"], spell_effect: [{effects}])"#
    )
}

#[test]
fn retained_cohorts_reject_missing_duplicate_early_and_noncanonical_bindings() {
    invalid(&spell(RETURN), "earlier producer");
    invalid(&spell(CAPTURE), "later consumer");
    invalid(&spell(&format!("{RETURN}, {CAPTURE}")), "earlier producer");
    invalid(
        &spell(&format!("{CAPTURE}, {CAPTURE}, {RETURN}")),
        "duplicate",
    );
    invalid(
        &spell(&format!("{CAPTURE}, {RETURN}, {RETURN}")),
        "one consumer",
    );
    invalid(
        &spell(&format!("{CAPTURE}, {RETURN}").replace("original_graveyards", "Bad-ID")),
        "canonical",
    );
}

#[test]
fn retained_cohorts_reject_narrowed_or_nonpermanent_filter_and_partial_player_set() {
    for capture in [
        CAPTURE.replace("Artifact", "Instant"),
        CAPTURE.replace("filter: Some((card_type: Some(Artifact)))", "filter: None"),
        CAPTURE.replace(
            "card_type: Some(Artifact)",
            "card_type: Some(Artifact), max_mana_value: Some(3)",
        ),
    ] {
        invalid(
            &spell(&format!("{capture}, {RETURN}")),
            "pure Artifact or Creature",
        );
    }
    invalid(
        &spell(&format!("{CAPTURE}, {RETURN}").replace("players: All", "players: Opponents")),
        "all-player",
    );
}

#[test]
fn retained_cohorts_reject_nested_conditional_and_ability_capture() {
    invalid(
        &spell(&format!(
            "Conditional(condition: ControllerLibraryEmpty, effect: {CAPTURE}), {RETURN}"
        )),
        "direct nonmodal",
    );
    let ron = format!(
        r#"(id: "cohort_probe", name: "Cohort Probe", face_id: "cohort_probe", types: ["Artifact"],
        activated_abilities: [(ability_id: "activated_01", presentation: Fallback, costs: [Tap], effect: [{CAPTURE}, {RETURN}])])"#
    );
    invalid(&ron, "spell instruction");
}

#[test]
fn scrap_mastery_and_living_death_shapes_retain_a_typed_exile_cohort() {
    for kind in ["Artifact", "Creature"] {
        let ron = spell(&format!(
            r#"
            ExileGraveyards(players: All, filter: Some((card_type: Some({kind}))),
                capture_exile_cohort: Some("original_graveyards")),
            SacrificeAll(players: All, filter: (kind: AnyPermanent, permanent_types: [{kind}])),
            ReturnExiledCohortToOwnersBattlefield(cohort_id: "original_graveyards"),
        "#
        ));
        let registry = CardRegistry::from_chunks_and_tokens(&[&ron], &[])
            .expect("the ordered cohort is retained across the middle sacrifice instruction");
        assert!(registry.get("cohort_probe").is_some());
        assert!(registry.get("living_death").is_none());
    }
}

#[test]
fn retained_cohorts_reject_modal_cross_face_and_nested_consumer_bindings() {
    let modal = format!(
        r#"(id:"cohort_probe",name:"Cohort Probe",face_id:"cohort_probe",mana_cost:"{{3}}{{R}}{{R}}",types:["Sorcery"],
        modal_spell:(min_modes:1,max_modes:1,modes:[(mode_id:"mode",presentation:Fallback,effects:[{CAPTURE},{RETURN}])]))"#
    );
    invalid(&modal, "direct nonmodal");
    let split = format!(
        r#"(id:"cohort_front_cohort_back",name:"Cohort Front // Cohort Back",layout:Split,faces:[
        (name:"Cohort Front",face_id:"cohort_front",mana_cost:"{{1}}{{R}}",types:["Sorcery"],spell_effect:[{CAPTURE}]),
        (name:"Cohort Back",face_id:"cohort_back",mana_cost:"{{1}}{{R}}",types:["Sorcery"],spell_effect:[{RETURN}])])"#
    );
    invalid(&split, "later consumer");
    invalid(
        &spell(&format!(
            "{CAPTURE},Conditional(condition:ControllerLibraryEmpty,effect:{RETURN})"
        )),
        "direct nonmodal",
    );
}
