use std::{
  collections::HashMap,
  sync::{OnceLock, RwLock},
};

fn cache() -> &'static RwLock<HashMap<String, &'static str>> {
  static CACHE: OnceLock<RwLock<HashMap<String, &'static str>>> = OnceLock::new();
  CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

pub(super) fn tr_static(key: &str) -> &'static str {
  if let Some(&interned) = cache().read().expect("market i18n cache poisoned").get(key) {
    return interned;
  }

  let resolved: &'static str = Box::leak(t!(key).into_owned().into_boxed_str());

  let mut cache = cache().write().expect("market i18n cache poisoned");
  cache.entry(key.to_owned()).or_insert(resolved)
}

#[cfg(test)]
mod tests {
  use super::*;

  mod tr_static {
    use super::*;

    #[test]
    fn it_resolves_a_market_key() {
      crate::services::i18n::set_locale(crate::services::i18n::Language::En);

      assert_eq!(tr_static("nav.market.browse"), "Browse");
    }

    #[test]
    fn it_returns_a_stable_pointer_for_a_repeated_key() {
      crate::services::i18n::set_locale(crate::services::i18n::Language::En);

      let first = tr_static("nav.market.orders");
      let second = tr_static("nav.market.orders");

      assert!(std::ptr::eq(first, second));
    }
  }
}
