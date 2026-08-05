#[path = "../build.rs"]
#[allow(dead_code)]
mod build_script;

#[test]
fn generated_module_names_accept_every_registry_id_shape() {
    assert_eq!(
        build_script::module_identifier("brainstorm"),
        "custom_effect_brainstorm"
    );
    assert_eq!(
        build_script::module_identifier("1996_world_champion"),
        "custom_effect_1996_world_champion"
    );
    assert_eq!(
        build_script::module_identifier("type"),
        "custom_effect_type"
    );
}
