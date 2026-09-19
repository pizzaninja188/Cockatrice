//! Registry assertions accept reviewed constants, never generator-produced expectations.
use tricerules_cards::{CardFace, CardRegistry, Keyword};

pub struct FaceExpectation<'a> {
    pub id: &'a str,
    pub name: &'a str,
    pub face_id: &'a str,
    pub mana_cost: &'a str,
    pub types: &'a [&'a str],
    pub keywords: &'a [Keyword],
    pub power_toughness: Option<(u32, u32)>,
}

impl FaceExpectation<'_> {
    pub fn check(&self) -> &'static CardFace {
        let id = self.id;
        let definition = CardRegistry::global()
            .get(id)
            .unwrap_or_else(|| panic!("missing {id}"));
        assert_eq!(definition.name, self.name, "{id}");
        let face = definition.primary_face();
        assert_eq!(face.face_id.as_str(), self.face_id, "{id}");
        assert_eq!(face.mana_cost.to_string(), self.mana_cost, "{id}");
        assert_eq!(
            face.types.iter().map(String::as_str).collect::<Vec<_>>(),
            self.types,
            "{id}"
        );
        assert_eq!(face.keywords, self.keywords, "{id}");
        let expected = self
            .power_toughness
            .map_or((None, None), |(p, t)| (Some(p), Some(t)));
        assert_eq!((face.power, face.toughness), expected, "{id}");
        face
    }
}
