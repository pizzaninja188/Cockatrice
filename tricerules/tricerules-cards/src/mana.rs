//! Structured mana costs in Scryfall brace syntax (`{4}{G}{G}`).
//!
//! Replaces the old per-character string where `"15"` parsed as six generic and
//! X/hybrid/Phyrexian were unrepresentable. Hand-authoring and Phase 6 codegen copy
//! `mana_cost` verbatim from Scryfall; the type serializes back to a plain RON string.

use crate::primitives::Color;
use serde::{Deserialize, Serialize};
use std::fmt;

/// A single mana symbol in a cost (CR 107.4). Only the symbols the engine can pay are
/// recognized; hybrid (`{G/U}`), Phyrexian (`{B/P}`) and snow (`{S}`) are expressible in
/// the brace syntax but rejected at parse time until the engine supports them.
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
}

/// An ordered list of mana symbols (CR 107.4). Serializes as the canonical brace string so
/// RON files keep a plain `mana_cost: "{1}{R}"` field.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ManaCost {
    pub pips: Vec<ManaSymbol>,
}

fn parse_symbol(token: &str) -> Result<ManaSymbol, String> {
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
                _ => 1,
            })
            .sum()
    }

    /// Colors implied by the cost's colored pips (CR 202.2a). Colorless/generic/X contribute none.
    pub fn colors(&self) -> Vec<Color> {
        let mut out = Vec::new();
        for p in &self.pips {
            let c = match p {
                ManaSymbol::W => Color::White,
                ManaSymbol::U => Color::Blue,
                ManaSymbol::B => Color::Black,
                ManaSymbol::R => Color::Red,
                ManaSymbol::G => Color::Green,
                _ => continue,
            };
            if !out.contains(&c) {
                out.push(c);
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
        let err = ManaCost::parse("{G/U}").unwrap_err();
        assert!(err.contains("unsupported mana symbol"), "{err}");
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
