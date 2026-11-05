//! IPC conversion and authoritative admission; the construction validator remains pure.
use crate::format_runtime::{self, Snapshot};
use crate::format_validation::{self, Deck, Format, Legality};
use tricerules_proto::{self as rv1, commander_setup, IpcResponse};

pub fn format(raw: i32) -> Option<Format> {
    match rv1::GameFormat::try_from(raw).ok()? {
        rv1::GameFormat::Standard => Some(Format::Standard),
        rv1::GameFormat::Commander => Some(Format::Commander),
        _ => None,
    }
}
pub fn deck(row: &rv1::PlayerDeck) -> Result<Deck, String> {
    let commanders = match row
        .commander_setup
        .as_ref()
        .and_then(|s| s.declaration.as_ref())
    {
        Some(commander_setup::Declaration::NoCommander(true)) => vec![],
        Some(commander_setup::Declaration::Declared(d)) => d.card_name.clone(),
        _ => return Err("An explicit commander declaration is required".into()),
    };
    Ok(Deck {
        mainboard: row.mainboard_card_name.clone(),
        sideboard: row.sideboard_card_name.clone(),
        commanders,
    })
}
pub fn validate(raw: i32, row: &rv1::PlayerDeck, snapshot: Option<&Snapshot>) -> IpcResponse {
    let mut report = rv1::DeckValidationReport {
        format: raw,
        legality: rv1::DeckLegality::Unknown as i32,
        ..Default::default()
    };
    let registry = tricerules_cards::registry::global();
    if let Some(snapshot) = snapshot {
        report.catalog_hash = snapshot.catalog.hash.clone();
        report.catalog_source_date = snapshot.catalog.source_date.clone();
        if snapshot.stale() {
            report.notice="The last successful catalogue check is over 48 hours old; validation uses the last good cached source".into();
        }
    }
    let input = deck(row);
    if let Ok(input) = &input {
        report.unsupported_card_names = input
            .mainboard
            .iter()
            .chain(&input.sideboard)
            .chain(&input.commanders)
            .filter(|name| registry.id_for_name(name).is_none())
            .cloned()
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect();
        report.engine_supported = report.unsupported_card_names.is_empty();
    }
    if let (Some(format), Ok(input), Some(snapshot)) = (format(raw), input.as_ref(), snapshot) {
        let result = format_validation::validate(&snapshot.catalog, format, input, |name| {
            registry.id_for_name(name).is_some()
        });
        report.legality = match result.legality {
            Legality::Legal => rv1::DeckLegality::Legal,
            Legality::Illegal => rv1::DeckLegality::Illegal,
            Legality::Unknown => rv1::DeckLegality::Unknown,
        } as i32;
        report.engine_supported = result.engine_supported;
        report.unsupported_card_names = result.unsupported_cards;
        report.issues = result
            .problems
            .into_iter()
            .map(|p| rv1::DeckValidationIssue {
                code: p.code,
                section: p.section,
                card_name: p.card_name,
                quantity: p.quantity,
                message: p.message,
            })
            .collect();
        for name in input
            .mainboard
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
        {
            if registry
                .id_for_name(name)
                .and_then(|id| registry.get(id))
                .is_some_and(|definition| definition.commander_setup_only)
            {
                report.engine_supported = false;
                if !report.unsupported_card_names.contains(name) {
                    report.unsupported_card_names.push(name.clone());
                }
                report.issues.push(rv1::DeckValidationIssue {
                    code: "engine_setup_only".into(),
                    section: "mainboard".into(),
                    card_name: name.clone(),
                    quantity: input
                        .mainboard
                        .iter()
                        .filter(|candidate| *candidate == name)
                        .count() as u32,
                    message: "This card is currently implemented only for commander setup".into(),
                });
            }
        }
    } else {
        let message = if format(raw).is_none() {
            "Choose Standard or Commander".to_owned()
        } else if let Err(message) = input {
            message
        } else {
            "No verified format catalogue is available; the server is fetching it. Try again after the update completes".into()
        };
        report.issues.push(rv1::DeckValidationIssue {
            code: "validation_unavailable".into(),
            message,
            ..Default::default()
        });
    }
    let accepted = report.legality == rv1::DeckLegality::Legal as i32 && report.engine_supported;
    IpcResponse {
        ok: accepted,
        error: if accepted {
            String::new()
        } else {
            "Deck admission failed; see the private validation report".into()
        },
        missing_card_names: report.unsupported_card_names.clone(),
        deck_validation: Some(report),
        ..Default::default()
    }
}
/// Return setup life and the pinned source. Replay never re-evaluates historical bans.
pub fn admit(
    start: &rv1::SessionStart,
    replay: bool,
) -> Result<(i32, String, String), Box<IpcResponse>> {
    let snapshot = if replay {
        None
    } else {
        format_runtime::current()
    };
    admit_with_snapshot(start, replay, snapshot.as_ref())
}
fn admit_with_snapshot(
    start: &rv1::SessionStart,
    replay: bool,
    snapshot: Option<&Snapshot>,
) -> Result<(i32, String, String), Box<IpcResponse>> {
    let Some(format) = format(start.format) else {
        // Historical trusted IPC fixtures have no format. Runtime entry points separately reject
        // this branch unless an explicit server-side test gate is enabled.
        if start.format == 0 {
            return Ok((20, String::new(), String::new()));
        }
        return Err(Box::new(IpcResponse {
            error: "Unknown game format".into(),
            ..Default::default()
        }));
    };
    let seats = start.player_ids.len();
    if (format == Format::Standard && seats != 2)
        || (format == Format::Commander && !(2..=4).contains(&seats))
        || start.player_decks.len() != seats
    {
        return Err(Box::new(IpcResponse {
            error: "Invalid format roster or missing player decks".into(),
            ..Default::default()
        }));
    }
    let life = if format == Format::Commander { 40 } else { 20 };
    if replay {
        if start.format_catalog_hash.len() != 64 || start.format_catalog_source_date.is_empty() {
            return Err(Box::new(IpcResponse {
                error: "Recorded format provenance is missing".into(),
                ..Default::default()
            }));
        }
        return Ok((
            life,
            start.format_catalog_hash.clone(),
            start.format_catalog_source_date.clone(),
        ));
    }
    for row in &start.player_decks {
        let response = validate(start.format, row, snapshot);
        if !response.ok {
            return Err(Box::new(response));
        }
    }
    let snapshot = snapshot.expect("successful validation requires a catalogue");
    if start.format_catalog_hash != snapshot.catalog.hash
        || start.format_catalog_source_date != snapshot.catalog.source_date
    {
        return Err(Box::new(IpcResponse {
            error: "Format catalogue changed since admission; ready up again".into(),
            ..Default::default()
        }));
    }
    Ok((
        life,
        snapshot.catalog.hash.clone(),
        snapshot.catalog.source_date.clone(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn snapshot() -> Snapshot {
        format_runtime::read_cache(
            &std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixtures/formats"),
        )
        .unwrap()
    }
    fn row(commander: bool) -> rv1::PlayerDeck {
        rv1::PlayerDeck {
            mainboard_card_name: vec![
                if commander {
                    "Island".into()
                } else {
                    "Forest".into()
                };
                if commander { 99 } else { 60 }
            ],
            commander_setup: Some(rv1::CommanderSetup {
                declaration: Some(if commander {
                    commander_setup::Declaration::Declared(commander_setup::Declared {
                        card_name: vec!["Kami of the Crescent Moon".into()],
                    })
                } else {
                    commander_setup::Declaration::NoCommander(true)
                }),
            }),
            ..Default::default()
        }
    }
    #[test]
    fn format_admission_reports_support_even_when_catalogue_is_unavailable() {
        let result = validate(rv1::GameFormat::Standard as i32, &row(false), None);
        let report = result.deck_validation.unwrap();
        assert!(!result.ok);
        assert_eq!(report.legality, rv1::DeckLegality::Unknown as i32);
        assert!(report.engine_supported);
    }
    #[test]
    fn format_admission_derives_setup_and_pins_one_catalogue() {
        let source = snapshot();
        for commander in [false, true] {
            let format = if commander {
                rv1::GameFormat::Commander
            } else {
                rv1::GameFormat::Standard
            };
            let a = row(commander);
            let mut b = a.clone();
            b.player_id = 1;
            let mut start = rv1::SessionStart {
                format: format as i32,
                player_ids: vec![0, 1],
                player_decks: vec![a, b],
                format_catalog_hash: source.catalog.hash.clone(),
                format_catalog_source_date: source.catalog.source_date.clone(),
                ..Default::default()
            };
            let setup = admit_with_snapshot(&start, false, Some(&source)).unwrap();
            assert_eq!(setup.0, if commander { 40 } else { 20 });
            assert_eq!(setup.1, source.catalog.hash);
            start.format_catalog_hash = "changed".into();
            assert!(admit_with_snapshot(&start, false, Some(&source)).is_err());
            start.format_catalog_hash = source.catalog.hash.clone();
            assert!(
                admit_with_snapshot(&start, true, None).is_ok(),
                "replay uses recorded provenance without current ban data"
            );
            start.player_decks[0].mainboard_card_name.pop();
            assert!(admit_with_snapshot(&start, false, Some(&source)).is_err());
        }
    }
    #[test]
    fn format_admission_uses_format_specific_roster_limits() {
        let source = snapshot();
        for seats in [1, 2, 3, 4, 5] {
            for commander in [false, true] {
                let start = rv1::SessionStart {
                    format: if commander {
                        rv1::GameFormat::Commander as i32
                    } else {
                        rv1::GameFormat::Standard as i32
                    },
                    player_ids: (0..seats).collect(),
                    player_decks: (0..seats)
                        .map(|id| {
                            let mut deck = row(commander);
                            deck.player_id = id;
                            deck
                        })
                        .collect(),
                    format_catalog_hash: source.catalog.hash.clone(),
                    format_catalog_source_date: source.catalog.source_date.clone(),
                    ..Default::default()
                };
                assert_eq!(
                    admit_with_snapshot(&start, false, Some(&source)).is_ok(),
                    if commander {
                        (2..=4).contains(&seats)
                    } else {
                        seats == 2
                    }
                );
            }
        }
    }
    #[test]
    fn format_runtime_rejects_missing_format_and_validation_does_not_touch_an_active_game() {
        use rv1::ipc_envelope::Msg;
        let envelope = rv1::IpcEnvelope {
            msg: Some(Msg::SessionStart(rv1::SessionStart {
                player_ids: vec![0, 1],
                ..Default::default()
            })),
        };
        let mut strict = crate::EngineSession::for_runtime(false, false);
        assert!(!strict.process(&envelope).unwrap().ok);
        assert!(strict.engine.is_none());
        let mut fixture = crate::EngineSession::new(false);
        assert!(fixture.process(&envelope).unwrap().ok);
        let before = fixture
            .engine
            .as_ref()
            .unwrap()
            .diagnostic_snapshot()
            .unwrap();
        let _ = fixture.process(&rv1::IpcEnvelope {
            msg: Some(Msg::ValidateDeck(rv1::ValidateDeck {
                format: rv1::GameFormat::Commander as i32,
                deck: Some(row(false)),
                ..Default::default()
            })),
        });
        assert_eq!(
            fixture
                .engine
                .as_ref()
                .unwrap()
                .diagnostic_snapshot()
                .unwrap(),
            before
        );
    }
}
