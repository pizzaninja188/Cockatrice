#![allow(clippy::derive_partial_eq_without_eq)]
#![allow(clippy::large_enum_variant)]

/// Prost emits flat items for `package ruled.v1`; nest them to match the proto package path.
pub mod ruled {
    pub mod v1 {
        include!(concat!(env!("OUT_DIR"), "/ruled.v1.rs"));
    }
}

pub use ruled::v1::*;

#[cfg(test)]
mod tests {
    use super::ruled::v1 as rv1;
    use prost::Message;

    #[test]
    fn presentation_reference_round_trips_every_identity_and_mapping_component() {
        let reference = rv1::PresentationRef {
            card_id: "grow_from_the_ashes".into(),
            face_id: "grow_from_the_ashes".into(),
            path: vec![
                rv1::PresentationPathComponent {
                    kind: rv1::PresentationPathKind::Spell as i32,
                    id: "spell".into(),
                },
                rv1::PresentationPathComponent {
                    kind: rv1::PresentationPathKind::CastCostGroup as i32,
                    id: "kicker".into(),
                },
                rv1::PresentationPathComponent {
                    kind: rv1::PresentationPathKind::CastCostOption as i32,
                    id: "kicked".into(),
                },
            ],
            oracle_line_indices: vec![1, 3],
            fallback_text: "Kicker {2}".into(),
            external_card_name: "Grow from the Ashes".into(),
            external_face_name: "Grow from the Ashes".into(),
            oracle_text_sha256: "0123456789abcdef".repeat(4),
        };

        let decoded = rv1::PresentationRef::decode(reference.encode_to_vec().as_slice())
            .expect("presentation reference decodes");
        assert_eq!(decoded, reference);
    }
}
