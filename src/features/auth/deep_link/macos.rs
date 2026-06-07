use objc2::{AllocAnyThread, define_class, rc::Retained, runtime::NSObject, sel};
use objc2_core_services::AEKeyword;
use objc2_foundation::{NSAppleEventDescriptor, NSAppleEventManager};

/// Apple Event class kInternetEventClass ('GURL', the four-char code 0x4755524C).
const INTERNET_EVENT_CLASS: u32 = 0x4755_524C;
/// Apple Event id kAEGetURL ('GURL'); shares the 'GURL' four-char code with the class.
const GET_URL_EVENT_ID: u32 = 0x4755_524C;
/// keyDirectObject ('----'), the descriptor keyword carrying the URL string.
const KEY_DIRECT_OBJECT: AEKeyword = 0x2D2D_2D2D;

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
  }
);

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
}
