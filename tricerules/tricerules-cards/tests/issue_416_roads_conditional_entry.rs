//! Issue #416 registry conformance for the five Roads conditional-entry identities.
//!
//! The entry-condition recipe (`static.enters_tapped.unless_control_mount_or_vehicle`) ships, but
//! the sacrifice-for-Pilot recipe stays unregistered until a typed Crew/Saddle contribution
//! modifier and `data/tokens/pilot_c_1_1.ron` exist (blocker #422). The created token's granted
//! static ("This token saddles Mounts and crews Vehicles as though its power were 2 greater") has
//! no shipped vocabulary, so the five identities stay fail-closed: no `Country Roads`-style card
//! can load while `pilot_c_1_1` is absent, and the generator cannot emit them because the
//! sacrifice clause is unmatched (guarded by
//! `issue_416_roads_stay_ungenerated_until_the_pilot_token_exists` in `gen_cards.rs`).
//!
//! Oracle and rulings checked 2026-09-19 (Scryfall `oracle_cards` snapshot
//! `27bf3214-1271-490b-bdfe-c0be6c23d02e`). CR 614.1d governs the entry replacement; CR 602.2 /
//! CR 118 govern the ordered activation costs; CR 111 governs tokens. The unimplemented
//! saddle/crew contribution static and the Pilot token are owned by blocker #422; Saddle itself
//! is blocker #319 (Saddle is CR 702.171 and Crew is CR 702.122 in the current rules; the issue's
//! "CR 702.166" citation is stale).

use tricerules_cards::CardRegistry;

const ROADS: &[(&str, &str)] = &[
    ("country_roads", "Country Roads"),
    ("foul_roads", "Foul Roads"),
    ("reef_roads", "Reef Roads"),
    ("rocky_roads", "Rocky Roads"),
    ("wild_roads", "Wild Roads"),
];

#[test]
fn issue_416_roads_identities_stay_unregistered_until_the_pilot_token_exists() {
    let registry = CardRegistry::global();
    for (id, name) in ROADS {
        assert!(
            registry.get(id).is_none(),
            "{name} must stay fail-closed until the Pilot token static (#422) ships"
        );
        assert_eq!(
            registry.id_for_name(name),
            None,
            "{name} must not resolve through the name index"
        );
    }
    assert!(
        !registry.is_token("pilot_c_1_1"),
        "the 1/1 colorless Pilot token must stay unregistered while its granted static is unsupported"
    );
}

/// Even if the generator emitted the exact typed payload today, the registry rejects it because
/// the Pilot token does not exist: the blocker fails closed at load time, not silently at play.
#[test]
fn issue_416_generated_roads_payload_is_rejected_without_the_pilot_token() {
    let probe = r#"(
        id: "issue_416_roads_probe",
        name: "Issue 416 Roads Probe",
        face_id: "issue_416_roads_probe",
        mana_cost: "",
        types: ["Land"],
        static_abilities: [(
            ability_id: "static_01",
            presentation: Fallback,
            definition: EntersTapped(
                affected: Self_,
                condition: Some(BattlefieldAggregate(
                    filter: (
                        controllers: Controller,
                        any_of: Some([
                            (controllers: Controller, required_subtypes: ["Mount"]),
                            (controllers: Controller, required_subtypes: ["Vehicle"]),
                        ]),
                    ),
                    aggregate: Count,
                    max: Some(0),
                )),
            ),
        )],
        activated_abilities: [
            (
                ability_id: "activated_01",
                presentation: Fallback,
                source_zone: Battlefield,
                costs: [Tap],
                effect: [ProduceMana(options: [(w: 1)])],
            ),
            (
                ability_id: "activated_02",
                presentation: Fallback,
                source_zone: Battlefield,
                costs: [Mana("{1}{W}"), Tap, SacrificeSelf],
                effect: [CreateTokens(token: "pilot_c_1_1", count: 1)],
                timing: SorcerySpeed,
            ),
        ],
    )"#;
    let error = CardRegistry::from_chunks_and_tokens(&[probe], &[])
        .expect_err("the generated Roads payload must fail closed while the Pilot token is absent");
    assert!(
        error.to_string().contains("pilot_c_1_1"),
        "the load failure must name the missing token: {error}"
    );
}
