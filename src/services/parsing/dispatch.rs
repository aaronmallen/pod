use thiserror::Error;

use crate::{
  features::{
    assets::{MultibuyMatch, MultibuyResolution, parse_multibuy},
    settings::facility_intel_fit::{ParsedFit, parse_fit},
    skills::{plan_csv, skill_plan_editor::parse_plan_text},
  },
  services::parsing::{eft, resolve::Resolver, sanitize},
};

#[derive(Debug, Eq, Error, PartialEq)]
pub enum ParseError {
  #[error("input did not match any known paste format")]
  Unrecognized,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Parsed {
  Fit(ParsedFit),
  Multibuy(Vec<(String, u64)>),
  Skills(Vec<(String, u8)>),
}

impl Parsed {
  pub async fn resolve(&self, resolver: &impl Resolver) -> Option<Resolved> {
    match self {
      Self::Fit(_) => None,
      Self::Multibuy(entries) => Some(Resolved::Multibuy(resolve_multibuy_entries(entries, resolver).await)),
      Self::Skills(rows) => Some(Resolved::Skills(resolve_skill_rows(rows, resolver).await)),
    }
  }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Resolved {
  Multibuy(MultibuyResolution),
  Skills(Vec<(i64, u8)>),
}

pub fn try_parse(text: &str) -> Result<Parsed, ParseError> {
  if looks_like_fit(text) {
    return Ok(Parsed::Fit(parse_fit(text, "", "", std::iter::empty::<(&str, i64)>())));
  }

  if let Some(skills) = detect_skills(text) {
    return Ok(Parsed::Skills(skills));
  }

  let items = parse_multibuy(text);
  if items.is_empty() {
    return Err(ParseError::Unrecognized);
  }

  Ok(Parsed::Multibuy(items))
}

fn detect_skills(text: &str) -> Option<Vec<(String, u8)>> {
  if let Some(rows) = plan_csv::parse(text) {
    return Some(rows);
  }

  let sanitized = sanitize::sanitize(text);
  let lines = sanitized.lines().filter(|line| !line.trim().is_empty()).count();
  if lines == 0 {
    return None;
  }

  let rows = parse_plan_text(&sanitized);
  (rows.len() == lines).then_some(rows)
}

async fn resolve_multibuy_entries(entries: &[(String, u64)], resolver: &impl Resolver) -> MultibuyResolution {
  if entries.is_empty() {
    return MultibuyResolution::default();
  }

  let names: Vec<String> = entries.iter().map(|(name, _)| name.clone()).collect();
  let resolution = resolver.resolve(&names).await;

  let mut matched = Vec::new();
  for (name, quantity) in entries {
    if let Some(&type_id) = resolution.matched.get(&name.to_lowercase()) {
      matched.push(MultibuyMatch {
        name: name.clone(),
        quantity: *quantity,
        type_id,
      });
    }
  }

  MultibuyResolution {
    matched,
    unmatched: resolution.unmatched,
  }
}

async fn resolve_skill_rows(rows: &[(String, u8)], resolver: &impl Resolver) -> Vec<(i64, u8)> {
  let names: Vec<String> = rows.iter().map(|(name, _)| name.clone()).collect();
  let resolution = resolver.resolve(&names).await;

  let mut resolved: Vec<(i64, u8)> = Vec::new();
  for (name, level) in rows {
    let Some(&skill_id) = resolution.matched.get(&name.to_lowercase()) else {
      continue;
    };
    match resolved.iter_mut().find(|(id, _)| *id == skill_id) {
      Some(entry) => entry.1 = entry.1.max(*level),
      None => resolved.push((skill_id, *level)),
    }
  }
  resolved
}

fn is_scan_section(line: &str) -> bool {
  let trimmed = line.trim().to_lowercase();
  trimmed.ends_with("slot") || trimmed.ends_with("slots")
}

fn looks_like_fit(text: &str) -> bool {
  text
    .lines()
    .any(|line| eft::parse_header(line).is_some() || is_scan_section(line))
}

#[cfg(test)]
mod tests {
  use super::*;

  const CARGO_SCAN: &str =
    "Standup M-Set Moon Drilling Stability I\t1\tRig Slot\nStandup Cloning Center I\t1\tService Slot";
  const EFT_FIT: &str = "[Rifter, Cheap Tackle]\n200mm AutoCannon I\n\nDamage Control I";
  const SHIP_SCAN: &str =
    "High Power Slots\nStandup Heavy Energy Neutralizer I\nRig Slots\nStandup M-Set Moon Drilling Stability I";

  mod try_parse {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_detects_an_eft_paste_as_a_fit() {
      match try_parse(EFT_FIT) {
        Ok(Parsed::Fit(fit)) => assert_eq!(fit.hull, Some("Rifter".to_owned())),
        other => panic!("expected a fit, got {other:?}"),
      }
    }

    #[test]
    fn it_detects_a_ship_scan_as_a_fit() {
      assert!(matches!(try_parse(SHIP_SCAN), Ok(Parsed::Fit(_))));
    }

    #[test]
    fn it_detects_a_cargo_scan_as_a_fit() {
      assert!(matches!(try_parse(CARGO_SCAN), Ok(Parsed::Fit(_))));
    }

    #[test]
    fn it_detects_roman_numeral_levels_as_skills() {
      match try_parse("Gunnery V\nDrones IV") {
        Ok(Parsed::Skills(rows)) => {
          assert_eq!(rows, vec![("Gunnery".to_owned(), 5), ("Drones".to_owned(), 4)]);
        }
        other => panic!("expected skills, got {other:?}"),
      }
    }

    #[test]
    fn it_detects_a_skills_csv_as_skills() {
      match try_parse("#,Skill,Group,Level,SP,Duration\n1,Gunnery,Gunnery,5,256000,1h\n") {
        Ok(Parsed::Skills(rows)) => assert_eq!(rows, vec![("Gunnery".to_owned(), 5)]),
        other => panic!("expected skills, got {other:?}"),
      }
    }

    #[test]
    fn it_falls_back_to_multibuy_for_quantities() {
      match try_parse("Tritanium\t1,000,000\nPyerite 250 000") {
        Ok(Parsed::Multibuy(rows)) => {
          assert_eq!(
            rows,
            vec![("Tritanium".to_owned(), 1_000_000), ("Pyerite".to_owned(), 250_000)]
          );
        }
        other => panic!("expected multibuy, got {other:?}"),
      }
    }

    #[test]
    fn it_errors_on_empty_input() {
      assert_eq!(try_parse(""), Err(ParseError::Unrecognized));
    }

    #[test]
    fn it_errors_on_whitespace_only_input() {
      assert_eq!(try_parse("   \n  \t\n"), Err(ParseError::Unrecognized));
    }
  }

  mod resolve {
    use std::collections::HashMap;

    use pretty_assertions::assert_eq;

    use super::*;
    use crate::services::parsing::resolve::Resolution;

    struct FakeResolver {
      matched: HashMap<String, i64>,
    }

    impl FakeResolver {
      fn new(pairs: &[(&str, i64)]) -> Self {
        Self {
          matched: pairs.iter().map(|(name, id)| ((*name).to_owned(), *id)).collect(),
        }
      }
    }

    impl Resolver for FakeResolver {
      async fn resolve(&self, names: &[String]) -> Resolution {
        let unmatched = names
          .iter()
          .filter(|name| !self.matched.contains_key(&name.to_lowercase()))
          .cloned()
          .collect();
        Resolution {
          matched: self.matched.clone(),
          unmatched,
        }
      }
    }

    #[tokio::test]
    async fn it_returns_none_for_a_fit() {
      let resolver = FakeResolver::new(&[]);

      assert_eq!(Parsed::Fit(ParsedFit::default()).resolve(&resolver).await, None);
    }

    #[tokio::test]
    async fn it_resolves_a_multibuy_into_matched_and_unmatched() {
      let resolver = FakeResolver::new(&[("tritanium", 34), ("pyerite", 35)]);
      let parsed = Parsed::Multibuy(vec![
        ("Tritanium".to_owned(), 100),
        ("Pyerite".to_owned(), 50),
        ("Notathing".to_owned(), 5),
      ]);

      let resolved = parsed.resolve(&resolver).await;

      assert_eq!(
        resolved,
        Some(Resolved::Multibuy(MultibuyResolution {
          matched: vec![
            MultibuyMatch {
              name: "Tritanium".to_owned(),
              quantity: 100,
              type_id: 34,
            },
            MultibuyMatch {
              name: "Pyerite".to_owned(),
              quantity: 50,
              type_id: 35,
            },
          ],
          unmatched: vec!["Notathing".to_owned()],
        }))
      );
    }

    #[tokio::test]
    async fn it_resolves_skills_and_keeps_the_highest_level_per_id() {
      let resolver = FakeResolver::new(&[("gunnery", 3300), ("drones", 3436)]);
      let parsed = Parsed::Skills(vec![
        ("Gunnery".to_owned(), 4),
        ("Drones".to_owned(), 5),
        ("gunnery".to_owned(), 5),
        ("Notaskill".to_owned(), 2),
      ]);

      let resolved = parsed.resolve(&resolver).await;

      assert_eq!(resolved, Some(Resolved::Skills(vec![(3300, 5), (3436, 5)])));
    }
  }

  mod detect_skills {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_returns_none_when_a_line_lacks_a_level_token() {
      assert_eq!(detect_skills("Gunnery V\nTritanium 1000000"), None);
    }

    #[test]
    fn it_returns_none_for_bare_multibuy_names() {
      assert_eq!(detect_skills("Tritanium\nPyerite"), None);
    }

    #[test]
    fn it_returns_none_for_empty_input() {
      assert_eq!(detect_skills("   \n"), None);
    }
  }

  mod is_scan_section {
    use super::*;

    #[test]
    fn it_matches_section_and_slot_lines() {
      assert!(is_scan_section("Rig Slots"));
      assert!(is_scan_section("Standup Cloning Center I\t1\tService Slot"));
    }

    #[test]
    fn it_rejects_ordinary_item_lines() {
      assert!(!is_scan_section("200mm AutoCannon I"));
    }
  }

  mod looks_like_fit {
    use super::*;

    #[test]
    fn it_matches_an_eft_header() {
      assert!(looks_like_fit(EFT_FIT));
    }

    #[test]
    fn it_rejects_plain_skill_and_multibuy_text() {
      assert!(!looks_like_fit("Gunnery V\nDrones IV"));
      assert!(!looks_like_fit("Tritanium 1000\nPyerite 500"));
    }
  }
}
