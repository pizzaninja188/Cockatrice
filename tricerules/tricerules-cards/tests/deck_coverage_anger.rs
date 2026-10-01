use tricerules_cards::primitives::StaticAbilityDef;
use tricerules_cards::{BasicLandType, CardRegistry, Keyword};

#[test]
fn anger_exact_definition_and_zone_specific_schema_boundaries() {
    let card = CardRegistry::global().get("anger").unwrap();
    let face = card.primary_face();
    assert_eq!(card.name, "Anger");
    assert_eq!(face.types, ["Creature", "Incarnation"]);
    assert_eq!((face.power, face.toughness), (Some(2), Some(2)));
    assert_eq!(face.keywords, [Keyword::Haste]);
    assert_eq!(face.static_abilities.len(), 1);
    assert!(matches!(
        face.static_abilities[0].definition,
        StaticAbilityDef::GraveyardAnthemKeyword {
            required_land_type: BasicLandType::Mountain,
            keyword: Keyword::Haste,
        }
    ));
    let presentation = ron::to_string(&face.static_abilities[0].presentation).unwrap();
    assert_eq!(presentation, "OracleLines([2])");
    for (land, keyword) in [
        ("Mountain", "Haste"),
        ("Forest", "Trample"),
        ("Island", "Flying"),
    ] {
        let definition =
            format!("GraveyardAnthemKeyword(required_land_type:{land},keyword:{keyword})");
        let parsed: StaticAbilityDef = ron::from_str(&definition).unwrap();
        let encoded = ron::to_string(&parsed).unwrap();
        assert_eq!(ron::from_str::<StaticAbilityDef>(&encoded).unwrap(), parsed);
    }
    assert!(ron::from_str::<StaticAbilityDef>(
        "GraveyardAnthemKeyword(required_land_type:Desert,keyword:Haste)"
    )
    .is_err());
    assert!(
        ron::from_str::<tricerules_cards::ResolvingEffectDuration>("WhileSourceInGraveyard")
            .is_err()
    );
    let draft = r#"(id:"probe",name:"Probe",face_id:"probe",types:["Instant"],
        spell_effect:[RemoveAllAbilities(subject:Chosen,duration:WhileSourceInGraveyard)])"#;
    assert!(CardRegistry::from_chunks_and_tokens(&[draft], &[]).is_err());
}
