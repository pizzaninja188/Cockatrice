//! Structured mana costs in Scryfall brace syntax (`{4}{G}{G}`).
//!
//! Replaces the old per-character string where `"15"` parsed as six generic and
//! X/hybrid/Phyrexian were unrepresentable. Hand-authoring and Phase 6 codegen copy
//! `mana_cost` verbatim from Scryfall; the type serializes back to a plain RON string.

use crate::primitives::Color;
use serde::{Deserialize, Serialize};
use std::fmt;

/// One of the five colored mana symbols (W U B R G). Used inside flexible pips (hybrid,
/// mono-hybrid, Phyrexian) so a pip can name its color(s) without nesting `ManaSymbol`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorPip {
    W,
    U,
    B,
    R,
    G,
}

impl ColorPip {
    fn parse(token: &str) -> Option<ColorPip> {
        Some(match token {
            "W" => ColorPip::W,
            "U" => ColorPip::U,
            "B" => ColorPip::B,
            "R" => ColorPip::R,
            "G" => ColorPip::G,
            _ => return None,
        })
    }

    /// The game color this pip contributes to a card's color identity (CR 202.2).
    pub fn color(self) -> Color {
        match self {
            ColorPip::W => Color::White,
            ColorPip::U => Color::Blue,
            ColorPip::B => Color::Black,
            ColorPip::R => Color::Red,
            ColorPip::G => Color::Green,
        }
    }

    fn letter(self) -> char {
        match self {
            ColorPip::W => 'W',
            ColorPip::U => 'U',
            ColorPip::B => 'B',
            ColorPip::R => 'R',
            ColorPip::G => 'G',
        }
    }
}

/// A single mana symbol in a cost (CR 107.4). Snow (`{S}`) is expressible in the brace syntax
/// but rejected at parse time until snow sources exist. Hybrid/mono-hybrid/Phyrexian pips
/// (CR 107.4d–f) are paid as a constrained choice — see `tricerules-core` `pay_mana`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManaSymbol {
    W,
    U,
    B,
    R,
    G,
    /// Colorless mana (`{C}`) — must be paid with colorless mana specifically, unlike generic.
    C,
    /// Generic mana of a fixed amount (`{4}`); a multi-digit value is one pip (fixes `"15"`).
    Generic(u32),
    /// Variable generic (`{X}`); CR 107.3. Representable but not castable yet.
    X,
    /// Hybrid (`{G/U}`, CR 107.4d): payable with one mana of either color. The card is both colors.
    Hybrid(ColorPip, ColorPip),
    /// Monocolored hybrid / "twobrid" (`{2/W}`, CR 107.4e): pay the generic amount OR one of the color.
    MonoHybrid(u32, ColorPip),
    /// Phyrexian (`{B/P}`, CR 107.4f): pay one mana of the color OR 2 life.
    Phyrexian(ColorPip),
}

/// An ordered list of mana symbols (CR 107.4). Serializes as the canonical brace string so
/// RON files keep a plain `mana_cost: "{1}{R}"` field.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ManaCost {
    pub pips: Vec<ManaSymbol>,
}

fn parse_symbol(token: &str) -> Result<ManaSymbol, String> {
    // Slash forms (CR 107.4d–f): {G/U} hybrid, {2/W} mono-hybrid, {B/P} Phyrexian. Three-part
    // forms ({G/U/P}) and snow ({S}) are not split here and fall through to the error below.
    if let Some((a, b)) = token.split_once('/') {
        let err = || format!("unsupported mana symbol {{{token}}}");
        if b == "P" {
            return Ok(ManaSymbol::Phyrexian(ColorPip::parse(a).ok_or_else(err)?));
        }
        let right = ColorPip::parse(b).ok_or_else(err)?;
        if let Ok(n) = a.parse::<u32>() {
            return Ok(ManaSymbol::MonoHybrid(n, right));
        }
        return Ok(ManaSymbol::Hybrid(
            ColorPip::parse(a).ok_or_else(err)?,
            right,
        ));
    }
    match token {
        "W" => Ok(ManaSymbol::W),
        "U" => Ok(ManaSymbol::U),
        "B" => Ok(ManaSymbol::B),
        "R" => Ok(ManaSymbol::R),
        "G" => Ok(ManaSymbol::G),
        "C" => Ok(ManaSymbol::C),
        "X" => Ok(ManaSymbol::X),
        _ => token
            .parse::<u32>()
            .map(ManaSymbol::Generic)
            .map_err(|_| format!("unsupported mana symbol {{{token}}}")),
    }
}

impl ManaCost {
    /// Strict parse of Scryfall brace syntax. The whole string must be `{...}` groups; an
    /// empty string is a free/no cost (lands). Unsupported symbols error by name.
    pub fn parse(s: &str) -> Result<Self, String> {
        let s = s.trim();
        if s.is_empty() {
            return Ok(ManaCost::default());
        }
        let mut pips = Vec::new();
        let mut rest = s;
        while !rest.is_empty() {
            let open = rest.strip_prefix('{').ok_or_else(|| {
                format!("mana cost must be brace groups like \"{{4}}{{G}}\", got {s:?}")
            })?;
            let end = open
                .find('}')
                .ok_or_else(|| format!("unterminated mana symbol in {s:?}"))?;
            pips.push(parse_symbol(&open[..end])?);
            rest = &open[end + 1..];
        }
        Ok(ManaCost { pips })
    }

    /// Mana value / converted mana cost (CR 202.3). `{X}` counts as 0 while not on the stack
    /// (CR 107.3).
    pub fn mana_value(&self) -> u32 {
        self.pips
            .iter()
            .map(|p| match p {
                ManaSymbol::Generic(n) => *n,
                ManaSymbol::X => 0,
                // CR 202.3f: a hybrid pip counts as the largest mana value among its alternatives,
                // so {2/W} is 2; {G/U} and {B/P} are 1 (life is not mana).
                ManaSymbol::MonoHybrid(n, _) => *n,
                _ => 1,
            })
            .sum()
    }

    /// Colors implied by the cost's colored pips (CR 202.2). Colorless/generic/X contribute none.
    /// A hybrid pip contributes *both* of its colors regardless of how it is paid (CR 202.2b);
    /// mono-hybrid and Phyrexian pips contribute their single color (CR 202.2c).
    pub fn colors(&self) -> Vec<Color> {
        let mut out = Vec::new();
        let mut push = |c: Color| {
            if !out.contains(&c) {
                out.push(c);
            }
        };
        for p in &self.pips {
            match p {
                ManaSymbol::W => push(Color::White),
                ManaSymbol::U => push(Color::Blue),
                ManaSymbol::B => push(Color::Black),
                ManaSymbol::R => push(Color::Red),
                ManaSymbol::G => push(Color::Green),
                ManaSymbol::Hybrid(a, b) => {
                    push(a.color());
                    push(b.color());
                }
                ManaSymbol::MonoHybrid(_, c) | ManaSymbol::Phyrexian(c) => push(c.color()),
                ManaSymbol::C | ManaSymbol::Generic(_) | ManaSymbol::X => {}
            }
        }
        out
    }

    /// No mana symbols at all (lands, "free" spells).
    pub fn is_empty(&self) -> bool {
        self.pips.is_empty()
    }

    /// True if the cost contains an `{X}` pip (cast not yet supported by the engine).
    pub fn has_x(&self) -> bool {
        self.pips.iter().any(|p| matches!(p, ManaSymbol::X))
    }
}

impl fmt::Display for ManaSymbol {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ManaSymbol::W => f.write_str("{W}"),
            ManaSymbol::U => f.write_str("{U}"),
            ManaSymbol::B => f.write_str("{B}"),
            ManaSymbol::R => f.write_str("{R}"),
            ManaSymbol::G => f.write_str("{G}"),
            ManaSymbol::C => f.write_str("{C}"),
            ManaSymbol::Generic(n) => write!(f, "{{{n}}}"),
            ManaSymbol::X => f.write_str("{X}"),
            ManaSymbol::Hybrid(a, b) => write!(f, "{{{}/{}}}", a.letter(), b.letter()),
            ManaSymbol::MonoHybrid(n, c) => write!(f, "{{{n}/{}}}", c.letter()),
            ManaSymbol::Phyrexian(c) => write!(f, "{{{}/P}}", c.letter()),
        }
    }
}

impl fmt::Display for ManaCost {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for p in &self.pips {
            write!(f, "{p}")?;
        }
        Ok(())
    }
}

impl TryFrom<String> for ManaCost {
    type Error = String;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        ManaCost::parse(&s)
    }
}

impl From<ManaCost> for String {
    fn from(c: ManaCost) -> String {
        c.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cost(s: &str) -> ManaCost {
        ManaCost::parse(s).unwrap()
    }

    #[test]
    fn empty_is_free() {
        assert!(cost("").is_empty());
        assert_eq!(cost("").mana_value(), 0);
    }

    #[test]
    fn parses_colored_and_generic() {
        assert_eq!(
            cost("{4}{G}{G}").pips,
            vec![ManaSymbol::Generic(4), ManaSymbol::G, ManaSymbol::G]
        );
    }

    #[test]
    fn multi_digit_generic_is_one_pip() {
        // The bug the brace syntax fixes: "15" used to read as 1+5=6.
        let c = cost("{15}");
        assert_eq!(c.pips, vec![ManaSymbol::Generic(15)]);
        assert_eq!(c.mana_value(), 15);
    }

    #[test]
    fn x_parses_but_values_zero() {
        let c = cost("{X}{R}");
        assert!(c.has_x());
        assert_eq!(c.mana_value(), 1); // X = 0 off the stack, {R} = 1
    }

    #[test]
    fn colorless_distinct_from_generic() {
        assert_eq!(cost("{C}").pips, vec![ManaSymbol::C]);
        assert!(cost("{C}").colors().is_empty());
    }

    #[test]
    fn colors_dedup_in_cost_order() {
        assert_eq!(cost("{1}{R}{R}").colors(), vec![Color::Red]);
        assert_eq!(cost("{W}{U}").colors(), vec![Color::White, Color::Blue]);
    }

    #[test]
    fn unsupported_symbol_errors_by_name() {
        // Snow and three-part hybrid-Phyrexian are still rejected.
        let err = ManaCost::parse("{S}").unwrap_err();
        assert!(err.contains("unsupported mana symbol"), "{err}");
        assert!(ManaCost::parse("{G/U/P}").is_err());
    }

    #[test]
    fn parses_hybrid_pips() {
        assert_eq!(
            cost("{G/U}").pips,
            vec![ManaSymbol::Hybrid(ColorPip::G, ColorPip::U)]
        );
        assert_eq!(
            cost("{2/W}").pips,
            vec![ManaSymbol::MonoHybrid(2, ColorPip::W)]
        );
        assert_eq!(cost("{B/P}").pips, vec![ManaSymbol::Phyrexian(ColorPip::B)]);
    }

    #[test]
    fn hybrid_mana_values_use_larger_alternative() {
        assert_eq!(cost("{G/U}").mana_value(), 1);
        assert_eq!(cost("{2/W}").mana_value(), 2);
        assert_eq!(cost("{B/P}").mana_value(), 1);
        assert_eq!(cost("{2/W}{2/W}").mana_value(), 4);
    }

    #[test]
    fn hybrid_colors_include_both_halves() {
        assert_eq!(cost("{G/U}").colors(), vec![Color::Green, Color::Blue]);
        assert_eq!(cost("{2/W}").colors(), vec![Color::White]);
        assert_eq!(cost("{B/P}").colors(), vec![Color::Black]);
    }

    #[test]
    fn hybrid_display_roundtrips() {
        for s in ["{G/U}", "{2/W}", "{B/P}", "{1}{G/U}{B/P}"] {
            assert_eq!(cost(s).to_string(), s);
        }
    }

    #[test]
    fn non_brace_input_errors() {
        assert!(ManaCost::parse("3B").is_err());
    }

    #[test]
    fn display_is_canonical_braces() {
        assert_eq!(cost("{4}{G}{G}").to_string(), "{4}{G}{G}");
        assert_eq!(cost("{0}").to_string(), "{0}");
    }

    #[test]
    fn serde_roundtrips_through_string() {
        let c = cost("{2}{U}{U}");
        let serialized = ron::to_string(&c).unwrap();
        assert_eq!(serialized, "\"{2}{U}{U}\"");
        let back: ManaCost = ron::from_str(&serialized).unwrap();
        assert_eq!(back, c);
    }
}
