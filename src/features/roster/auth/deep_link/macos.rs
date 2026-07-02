use objc2::{AllocAnyThread, define_class, rc::Retained, runtime::NSObject, sel};
use objc2_core_services::{AEKeyword, DescType};
use objc2_foundation::{NSAppleEventDescriptor, NSAppleEventManager};

/// Apple Event class kInternetEventClass ('GURL', the four-char code 0x4755524C).
const INTERNET_EVENT_CLASS: u32 = 0x4755_524C;
/// Apple Event id kAEGetURL ('GURL'); shares the 'GURL' four-char code with the class.
const GET_URL_EVENT_ID: u32 = 0x4755_524C;
/// keyDirectObject ('----'), the descriptor keyword carrying the URL string.
const KEY_DIRECT_OBJECT: AEKeyword = 0x2D2D_2D2D;
/// Apple Event class kCoreEventClass ('aevt', the four-char code 0x61657674).
const CORE_EVENT_CLASS: u32 = 0x6165_7674;
/// Apple Event id kAEOpenDocuments ('odoc', the four-char code 0x6F646F63).
const OPEN_DOCUMENTS_EVENT_ID: u32 = 0x6F64_6F63;
/// typeFileURL ('furl'), the descriptor type each opened document coerces to.
const TYPE_FILE_URL: DescType = 0x6675_726C;

define_class!(
  #[unsafe(super(NSObject))]
  #[name = "PodDeepLinkHandler"]
  struct Handler;

  impl Handler {
    #[unsafe(method(handleGetURLEvent:withReplyEvent:))]
    fn handle_get_url(&self, event: Option<&NSAppleEventDescriptor>, _reply: Option<&NSAppleEventDescriptor>) {
      if let Some(url) = extract_url(event) {
        super::deliver(url);
      }
    }

    #[unsafe(method(handleOpenDocumentsEvent:withReplyEvent:))]
    fn handle_open_documents(&self, event: Option<&NSAppleEventDescriptor>, _reply: Option<&NSAppleEventDescriptor>) {
      for path in extract_file_paths(event) {
        super::deliver_file(path);
      }
    }
  }
);

fn extract_file_paths(event: Option<&NSAppleEventDescriptor>) -> Vec<String> {
  let Some(list) = event.and_then(|event| event.paramDescriptorForKeyword(KEY_DIRECT_OBJECT)) else {
    return Vec::new();
  };
  // AEDescList items are 1-indexed, not 0-indexed.
  (1..=list.numberOfItems())
    .filter_map(|index| file_path_at(&list, index))
    .collect()
}

fn file_path_at(list: &NSAppleEventDescriptor, index: isize) -> Option<String> {
  let item = list.descriptorAtIndex(index)?;
  let url = item.coerceToDescriptorType(TYPE_FILE_URL)?.stringValue()?.to_string();
  Some(file_url_to_path(&url))
}

/// Percent-decodes byte-by-byte, not char-by-char: a non-ASCII path component is escaped as
/// consecutive `%XX` triples per UTF-8 byte, so the bytes must be reassembled before the
/// (possibly still-invalid) result is lossily converted back to a `String`.
fn file_url_to_path(url: &str) -> String {
  let trimmed = url.strip_prefix("file://").unwrap_or(url);
  let mut bytes = Vec::with_capacity(trimmed.len());
  let mut chars = trimmed.bytes();
  while let Some(byte) = chars.next() {
    if byte == b'%'
      && let (Some(high), Some(low)) = (chars.next(), chars.next())
      && let (Some(high), Some(low)) = ((high as char).to_digit(16), (low as char).to_digit(16))
    {
      bytes.push((high * 16 + low) as u8);
    } else {
      bytes.push(byte);
    }
  }
  String::from_utf8_lossy(&bytes).into_owned()
}

fn extract_url(event: Option<&NSAppleEventDescriptor>) -> Option<String> {
  let direct = event?.paramDescriptorForKeyword(KEY_DIRECT_OBJECT)?;
  Some(direct.stringValue()?.to_string())
}

pub fn install() {
  let handler = Handler::alloc().set_ivars(());
  let handler: Retained<Handler> = unsafe { objc2::msg_send![super(handler), init] };
  let manager = NSAppleEventManager::sharedAppleEventManager();
  unsafe {
    manager.setEventHandler_andSelector_forEventClass_andEventID(
      &handler,
      sel!(handleGetURLEvent:withReplyEvent:),
      INTERNET_EVENT_CLASS,
      GET_URL_EVENT_ID,
    );
    manager.setEventHandler_andSelector_forEventClass_andEventID(
      &handler,
      sel!(handleOpenDocumentsEvent:withReplyEvent:),
      CORE_EVENT_CLASS,
      OPEN_DOCUMENTS_EVENT_ID,
    );
  }
  std::mem::forget(handler);
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn extract_url_returns_none_for_a_null_event() {
    assert!(extract_url(None).is_none());
  }

  #[test]
  fn extract_file_paths_returns_empty_for_a_null_event() {
    assert!(extract_file_paths(None).is_empty());
  }

  mod file_url_to_path {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_strips_the_file_scheme_and_percent_decodes_the_path() {
      assert_eq!(
        file_url_to_path("file:///Users/me/My%20Packs/rules.pbr"),
        "/Users/me/My Packs/rules.pbr"
      );
    }

    #[test]
    fn it_returns_a_bare_posix_path_unchanged() {
      assert_eq!(file_url_to_path("/Users/me/plan.psp"), "/Users/me/plan.psp");
    }
  }
}
