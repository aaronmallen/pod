//! Shared utility functions for the skill picker tabs.

use pod_model::ItemTypeSummary;

/// Groups items by their `group_name`, optionally filtering by a search query.
///
/// Items whose name does not contain `lc` (lowercased search string) are
/// skipped when `searching` is `true`. Groups are returned in the order they
/// are first encountered.
pub fn collect_item_groups<'a>(
  items: &'a [ItemTypeSummary],
  lc: &str,
  searching: bool,
) -> Vec<(&'a str, Vec<&'a ItemTypeSummary>)> {
  let mut groups: Vec<(&str, Vec<&ItemTypeSummary>)> = Vec::new();
  for item in items {
    if searching && !item.name.to_lowercase().contains(lc) {
      continue;
    }
    match groups.iter_mut().find(|(g, _)| *g == item.group_name.as_str()) {
      Some((_, members)) => members.push(item),
      None => groups.push((item.group_name.as_str(), vec![item])),
    }
  }
  groups
}
