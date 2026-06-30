use iced::{
  Background, Element, Font, Length, Padding,
  alignment::Vertical,
  font,
  widget::{Column, Row, Space, container, rich_text, scrollable, span, text},
};

use super::{Message, ReadingRender, loaders::MessageLabel};
use crate::{
  store::model::character_mail_view::MailRender,
  ui::{
    components::{
      avatar::Avatar,
      button::{Button, Size},
      chip::label_chip,
      empty_state::empty_state as shared_empty_state,
      icon::Icon,
      rule,
    },
    style::{color, radius, spacing, typography},
  },
};

const TOOLBAR_SIDE_PADDING: f32 = 24.0;

const BODY_MAX_WIDTH: f32 = 720.0;

const SENDER_AVATAR_SIZE: f32 = 44.0;

pub(super) fn pane(render: Option<&ReadingRender>, is_snoozed: bool, in_trash: bool) -> Element<'_, Message> {
  let body: Element<'_, Message> = match render {
    Some(render) => opened(render, is_snoozed, in_trash),
    None => empty_state(),
  };

  container(body)
    .width(Length::Fill)
    .height(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::surface::BASE)),
      ..container::Style::default()
    })
    .into()
}

fn empty_state<'a>() -> Element<'a, Message> {
  shared_empty_state(super::tr_static("mail.reading.empty")).render()
}

fn opened(render: &ReadingRender, is_snoozed: bool, in_trash: bool) -> Element<'_, Message> {
  Column::with_children(vec![toolbar(render, is_snoozed, in_trash), scroll_body(render)])
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn toolbar(render: &ReadingRender, is_snoozed: bool, in_trash: bool) -> Element<'_, Message> {
  let mail_id = render.mail.header.mail_id();

  let star_label = if render.is_starred {
    t!("mail.reading.toolbar.starred")
  } else {
    t!("mail.reading.toolbar.star")
  };
  let snooze_label = if is_snoozed {
    t!("mail.reading.toolbar.snoozed")
  } else {
    t!("mail.reading.toolbar.snooze")
  };
  let (trash_label, trash_message) = if in_trash {
    (t!("mail.reading.toolbar.delete"), Message::Delete(mail_id))
  } else {
    (t!("mail.reading.toolbar.trash"), Message::Trash(mail_id))
  };

  let mut row = Row::new().align_y(Vertical::Center);
  row = row.push(toolbar_button(
    Icon::reply(),
    &t!("mail.reading.toolbar.reply"),
    Message::Reply(mail_id),
    false,
  ));
  row = row.push(toolbar_button(
    Icon::reply_all(),
    &t!("mail.reading.toolbar.reply_all"),
    Message::ReplyAll(mail_id),
    false,
  ));
  row = row.push(toolbar_button(
    Icon::forward(),
    &t!("mail.reading.toolbar.forward"),
    Message::Forward(mail_id),
    false,
  ));
  row = row.push(toolbar_button(
    Icon::tag(),
    &t!("mail.reading.toolbar.label"),
    Message::LabelPickerOpened(mail_id),
    false,
  ));
  row = row.push(toolbar_divider());
  row = row.push(toolbar_button(
    Icon::star(),
    &star_label,
    Message::ToggleStar(mail_id),
    false,
  ));
  row = row.push(toolbar_button(
    Icon::snooze(),
    &snooze_label,
    Message::SnoozeMenuToggled,
    false,
  ));
  row = row.push(toolbar_button(
    Icon::archive(),
    &t!("mail.reading.toolbar.archive"),
    Message::Archive(mail_id),
    false,
  ));
  row = row.push(toolbar_button(Icon::trash(), &trash_label, trash_message, true));
  row = row.push(Space::new().width(Length::Fill));
  row = row.push(timestamp_stamp(render.mail.header.timestamp().clone()));

  let bar = container(row).width(Length::Fill).padding(Padding {
    top: spacing::SPACE_2,
    bottom: spacing::SPACE_2,
    left: spacing::SPACE_2 * 2.0,
    right: TOOLBAR_SIDE_PADDING,
  });

  Column::with_children(vec![bar.into(), rule::horizontal()])
    .width(Length::Fill)
    .into()
}

fn toolbar_button<'a>(icon: Icon, label: &str, message: Message, danger: bool) -> Element<'a, Message> {
  let button = if danger {
    Button::danger(label.to_owned())
  } else {
    Button::ghost(label.to_owned())
  };

  button.icon(icon).size(Size::Sm).on_press(message).into()
}

fn toolbar_divider<'a>() -> Element<'a, Message> {
  container(rule::vertical(18.0))
    .padding(Padding {
      top: 0.0,
      bottom: 0.0,
      left: spacing::UNIT + 2.0,
      right: spacing::UNIT + 2.0,
    })
    .into()
}

fn timestamp_stamp<'a>(timestamp: String) -> Element<'a, Message> {
  text(timestamp)
    .font(typography::mono::REGULAR)
    .size(typography::size::XS_PLUS)
    .style(|_| text::Style {
      color: Some(color::text::secondary()),
    })
    .into()
}

fn scroll_body(render: &ReadingRender) -> Element<'_, Message> {
  let mail = &render.mail;
  let mut column = Column::new().width(Length::Fill).max_width(BODY_MAX_WIDTH);

  if let Some(chips) = label_chips(&render.labels) {
    column = column.push(chips);
  }
  column = column.push(subject(mail));
  column = column.push(sender_block(render));
  column = column.push(body_paragraphs(mail));

  let inner = container(column).width(Length::Fill).padding(Padding {
    top: spacing::SPACE_6 + spacing::SPACE_2,
    bottom: spacing::SPACE_6 * 2.0,
    left: spacing::SPACE_6 * 2.0,
    right: spacing::SPACE_6 * 2.0,
  });

  scrollable(inner)
    .style(crate::ui::style::control::scrollbar)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn label_chips(labels: &[MessageLabel]) -> Option<Element<'_, Message>> {
  if labels.is_empty() {
    return None;
  }
  let mut chips = Row::new().spacing(spacing::SPACE_2 - 2.0);
  for label in labels {
    chips = chips.push(label_chip::<Message>(&label.name, label.color.as_deref()));
  }
  Some(
    container(chips)
      .padding(Padding {
        top: 0.0,
        bottom: spacing::SPACE_2 * 2.0,
        left: 0.0,
        right: 0.0,
      })
      .into(),
  )
}

fn subject(mail: &MailRender) -> Element<'_, Message> {
  let subject = mail
    .header
    .subject()
    .clone()
    .filter(|s| !s.trim().is_empty())
    .unwrap_or_else(|| t!("mail.reading.no_subject").into_owned());

  container(
    text(subject)
      .size(typography::size::LG)
      .font(typography::body::MEDIUM)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      }),
  )
  .padding(Padding {
    top: 0.0,
    bottom: spacing::SPACE_6,
    left: 0.0,
    right: 0.0,
  })
  .into()
}

fn sender_block(render: &ReadingRender) -> Element<'_, Message> {
  let mail = &render.mail;
  let sender = mail.header.from_name();
  let is_system = mail.recipients.iter().any(|r| r.recipient_type() == "mailing_list") || mail.header.from_id() == 0;

  let avatar = sender_avatar(mail.header.from_id(), sender, render.sender_portrait.path());

  let to_line = {
    let mut row = Row::new().spacing(spacing::UNIT).align_y(Vertical::Center);
    row = row.push(
      text(t!("mail.reading.to"))
        .font(typography::mono::REGULAR)
        .size(typography::size::XS_PLUS)
        .style(|_| text::Style {
          color: Some(color::text::secondary()),
        }),
    );
    row = row.push(
      text(recipients_label(mail))
        .font(typography::mono::REGULAR)
        .size(typography::size::XS_PLUS)
        .style(|_| text::Style {
          color: Some(color::text::secondary()),
        }),
    );
    if is_system {
      row = row.push(
        text(t!("mail.reading.system_message"))
          .font(typography::mono::REGULAR)
          .size(typography::size::XS_PLUS)
          .style(|_| text::Style {
            color: Some(color::text::tertiary()),
          }),
      );
    }
    row
  };

  let names = Column::with_children(vec![
    text(sender.clone())
      .size(typography::size::MD)
      .font(typography::body::MEDIUM)
      .style(|_| text::Style {
        color: Some(color::text::PRIMARY),
      })
      .into(),
    container(to_line)
      .padding(Padding {
        top: spacing::UNIT,
        bottom: 0.0,
        left: 0.0,
        right: 0.0,
      })
      .into(),
  ])
  .width(Length::Fill);

  let time = text(mail.header.timestamp().clone())
    .font(typography::mono::REGULAR)
    .size(typography::size::SM)
    .style(|_| text::Style {
      color: Some(color::text::secondary()),
    });

  let block = Row::with_children(vec![avatar, names.into(), time.into()])
    .spacing(spacing::SPACE_3_5)
    .align_y(Vertical::Center)
    .width(Length::Fill);

  let block = container(block).width(Length::Fill).padding(Padding {
    top: 0.0,
    bottom: spacing::SPACE_6 + spacing::UNIT,
    left: 0.0,
    right: 0.0,
  });

  Column::with_children(vec![block.into(), rule::horizontal()])
    .width(Length::Fill)
    .into()
}

fn recipients_label(mail: &MailRender) -> String {
  if mail.recipients_display.trim().is_empty() {
    t!("mail.reading.recipients_self").into_owned()
  } else {
    mail.recipients_display.clone()
  }
}

fn sender_avatar(sender_id: i64, name: &str, portrait: Option<std::path::PathBuf>) -> Element<'_, Message> {
  Avatar::new(
    sender_id,
    name.to_owned(),
    Length::Fixed(SENDER_AVATAR_SIZE),
    SENDER_AVATAR_SIZE,
    portrait,
  )
  .border(color::with_alpha(color::text::PRIMARY, 0.1), 1.0)
  .radius(radius::CONTROL)
  .view::<Message>()
}

const BODY_SIZE: f32 = typography::size::MD;

#[derive(Clone, Debug, Default, PartialEq)]
struct BodySpan {
  text: String,
  size: f32,
  color: Option<iced::Color>,
  bold: bool,
  italic: bool,
  underline: bool,
  link: bool,
}

#[derive(Clone, Debug)]
struct BodyStyle {
  size: f32,
  color: Option<iced::Color>,
  bold: bool,
  italic: bool,
  underline: bool,
  link: bool,
}

impl Default for BodyStyle {
  fn default() -> Self {
    Self {
      size: BODY_SIZE,
      color: None,
      bold: false,
      italic: false,
      underline: false,
      link: false,
    }
  }
}

#[derive(Clone, Debug)]
struct StyleFrame {
  tag: String,
  style: BodyStyle,
}

fn body_paragraphs(mail: &MailRender) -> Element<'_, Message> {
  let spans = parse_stored_body(mail.body.body());

  if spans.iter().all(|s| s.text.trim().is_empty()) {
    return text(t!("mail.reading.no_content"))
      .size(BODY_SIZE)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      })
      .into();
  }

  let spans: Vec<iced::widget::text::Span<'_, ()>> = spans
    .into_iter()
    .map(|s| {
      let color = if s.link {
        Some(color::accent::PLASMA)
      } else {
        s.color.or(Some(color::with_alpha(color::text::PRIMARY, 0.88)))
      };
      span(s.text)
        .size(s.size)
        .color_maybe(color)
        .font(body_font(s.bold, s.italic))
        .underline(s.underline || s.link)
    })
    .collect();

  rich_text(spans).size(BODY_SIZE).width(Length::Fill).into()
}

/// Picks a Space Grotesk face for the requested weight/slant.
///
/// iced synthesizes bold and italic from the base family when a dedicated face is not embedded, so
/// requesting `Weight::Bold` / `Style::Italic` still yields the in-game emphasis.
fn body_font(bold: bool, italic: bool) -> Font {
  Font {
    family: font::Family::Name("Space Grotesk"),
    weight: if bold { font::Weight::Bold } else { font::Weight::Normal },
    stretch: font::Stretch::Normal,
    style: if italic {
      font::Style::Italic
    } else {
      font::Style::Normal
    },
  }
}

/// Tokenizes EVE's HTML-like mail markup into a flat list of styled [`BodySpan`]s.
///
/// The tokenizer is deliberately forgiving: it maintains a style stack, tolerates misnested and
/// stray closing tags, and never drops text. Unknown tags are skipped. `<loc>` wrappers and link
/// hrefs carry no visible styling of their own beyond the link emphasis applied at render time.
fn parse_stored_body(html: &str) -> Vec<BodySpan> {
  let mut stack: Vec<StyleFrame> = Vec::new();
  let mut spans: Vec<BodySpan> = Vec::new();
  let mut buffer = String::new();

  let bytes = html.as_bytes();
  let mut i = 0;
  while i < bytes.len() {
    i = if bytes[i] == b'<' {
      consume_tag(html, bytes, i, &mut stack, &mut spans, &mut buffer)
    } else {
      consume_text(html, i, &mut buffer)
    };
  }

  flush(&stack, &mut spans, &mut buffer);
  spans
}

fn consume_tag(
  html: &str,
  bytes: &[u8],
  mut i: usize,
  stack: &mut Vec<StyleFrame>,
  spans: &mut Vec<BodySpan>,
  buffer: &mut String,
) -> usize {
  let start = i;
  while i < bytes.len() && bytes[i] != b'>' {
    i += 1;
  }
  let raw = &html[start..i.min(html.len())];
  if i < bytes.len() {
    i += 1; // consume the closing '>'
  }
  apply_tag(raw, stack, spans, buffer);
  i
}

fn consume_text(html: &str, i: usize, buffer: &mut String) -> usize {
  let Some(c) = html[i..].chars().next() else {
    return i + 1;
  };
  if c == '\r' {
    return i + 1;
  }
  buffer.push(c);
  i + c.len_utf8()
}

fn apply_tag(raw: &str, stack: &mut Vec<StyleFrame>, spans: &mut Vec<BodySpan>, buffer: &mut String) {
  let inner = raw.trim_start_matches('<').trim_end_matches('>').trim();
  let lower = inner.to_ascii_lowercase();
  let name = lower
    .trim_start_matches('/')
    .split([' ', '\t', '\n', '/'])
    .next()
    .unwrap_or("");
  let closing = lower.starts_with('/');

  match name {
    "br" => buffer.push('\n'),
    "p" | "div" if closing => buffer.push('\n'),
    "p" | "div" => {}
    "b" | "i" | "u" | "font" | "a" => {
      flush(stack, spans, buffer);
      if closing {
        pop_style(stack, name);
      } else {
        push_style(stack, name, inner);
      }
    }
    _ => {}
  }
}

fn push_style(stack: &mut Vec<StyleFrame>, name: &str, inner: &str) {
  let mut style = stack
    .last()
    .map_or_else(BodyStyle::default, |frame| frame.style.clone());
  match name {
    "b" => style.bold = true,
    "i" => style.italic = true,
    "u" => style.underline = true,
    "a" => style.link = true,
    "font" => {
      if let Some(size) = attr_value(inner, "size").and_then(|v| v.parse::<f32>().ok())
        && size > 0.0
      {
        style.size = size;
      }
      if let Some(color) = attr_value(inner, "color").and_then(|v| color::from_argb(&v)) {
        style.color = Some(color);
      }
    }
    _ => {}
  }
  stack.push(StyleFrame {
    tag: name.to_owned(),
    style,
  });
}

/// Pops the nearest frame opened by `name`, tolerating stray or misnested closing tags.
///
/// A stray close with no matching open is ignored. A matching open buried under unclosed inner
/// frames discards those too, so misnested `</loc>`/`</a>` interleavings never strand later text.
fn pop_style(stack: &mut Vec<StyleFrame>, name: &str) {
  if let Some(index) = stack.iter().rposition(|frame| frame.tag == name) {
    stack.truncate(index);
  }
}

fn flush(stack: &[StyleFrame], spans: &mut Vec<BodySpan>, buffer: &mut String) {
  if buffer.is_empty() {
    return;
  }
  let style = stack
    .last()
    .map_or_else(BodyStyle::default, |frame| frame.style.clone());
  spans.push(BodySpan {
    text: std::mem::take(buffer),
    size: style.size,
    color: style.color,
    bold: style.bold,
    italic: style.italic,
    underline: style.underline,
    link: style.link,
  });
}

fn attr_value(inner: &str, attr: &str) -> Option<String> {
  let lower = inner.to_ascii_lowercase();
  let mut search = 0;
  while let Some(rel) = lower[search..].find(attr) {
    let at = search + rel;
    let after = at + attr.len();
    let rest = inner[after..].trim_start();
    // Guard against matching a longer attribute name that merely contains `attr`.
    let boundary_ok = inner[..at]
      .chars()
      .next_back()
      .is_none_or(|c| !c.is_ascii_alphanumeric());
    if boundary_ok && rest.starts_with('=') {
      let value = rest[1..].trim_start();
      let value = value.trim_start_matches(['"', '\'']);
      let end = value.find(['"', '\'']).unwrap_or(value.len());
      return Some(value[..end].to_owned());
    }
    search = after;
  }
  None
}

#[cfg(test)]
mod tests {
  use pretty_assertions::assert_eq;

  use super::*;

  fn rendered(html: &str) -> String {
    parse_stored_body(html).iter().map(|s| s.text.as_str()).collect()
  }

  #[test]
  fn it_preserves_paragraph_text_across_block_tags() {
    let html = "<p>First line.</p><p>Second line here.</p>";
    assert_eq!(rendered(html), "First line.\nSecond line here.\n");
  }

  #[test]
  fn it_keeps_plain_text_for_bold_and_italic_runs() {
    let spans = parse_stored_body("Form up at <b>Jita</b> by <i>19:15</i>.");
    assert_eq!(
      rendered("Form up at <b>Jita</b> by <i>19:15</i>."),
      "Form up at Jita by 19:15."
    );
    let jita = spans.iter().find(|s| s.text == "Jita").expect("bold span");
    assert!(jita.bold && !jita.italic);
    let time = spans.iter().find(|s| s.text == "19:15").expect("italic span");
    assert!(time.italic && !time.bold);
  }

  #[test]
  fn it_emits_newlines_for_br_tags() {
    assert_eq!(rendered("Alpha<br><br>Beta<br>"), "Alpha\n\nBeta\n");
  }

  #[test]
  fn it_yields_no_spans_for_an_empty_body() {
    assert!(parse_stored_body("").is_empty());
  }

  #[test]
  fn it_skips_carriage_returns_in_body_text() {
    assert_eq!(rendered("a\r\nb"), "a\nb");
  }

  #[test]
  fn it_underlines_u_runs() {
    let spans = parse_stored_body("plain <u>under</u> plain");
    let underlined = spans.iter().find(|s| s.text == "under").expect("underline span");
    assert!(underlined.underline);
  }

  #[test]
  fn it_applies_font_color_dropping_the_alpha_byte() {
    let spans = parse_stored_body(r##"<font color="#ffff0000">red</font>"##);
    let red = spans.iter().find(|s| s.text == "red").expect("colored span");
    assert_eq!(red.color, Some(iced::Color::from_rgb8(255, 0, 0)));
  }

  #[test]
  fn it_applies_per_span_font_size() {
    let spans = parse_stored_body(r##"<font size="24">big</font>"##);
    let big = spans.iter().find(|s| s.text == "big").expect("sized span");
    assert_eq!(big.size, 24.0);
  }

  #[test]
  fn it_inherits_outer_font_attributes_into_nested_runs() {
    let spans = parse_stored_body(r##"<font size="18" color="#ff00ff00"><b>x</b></font>"##);
    let nested = spans.iter().find(|s| s.text == "x").expect("nested span");
    assert_eq!(nested.size, 18.0);
    assert_eq!(nested.color, Some(iced::Color::from_rgb8(0, 255, 0)));
    assert!(nested.bold);
  }

  #[test]
  fn it_restores_the_outer_style_after_a_nested_run_closes() {
    let spans = parse_stored_body(r##"<font size="18">a<b>b</b>c</font>"##);
    let a = spans.iter().find(|s| s.text == "a").expect("a");
    let b = spans.iter().find(|s| s.text == "b").expect("b");
    let c = spans.iter().find(|s| s.text == "c").expect("c");
    assert!(!a.bold && a.size == 18.0);
    assert!(b.bold && b.size == 18.0);
    assert!(!c.bold && c.size == 18.0);
  }

  #[test]
  fn it_marks_link_inner_text_without_dropping_it() {
    let spans = parse_stored_body(r##"<a href="showinfo:601">Ibis</a>"##);
    let link = spans.iter().find(|s| s.text == "Ibis").expect("link span");
    assert!(link.link);
  }

  #[test]
  fn it_drops_loc_wrappers_without_artifacts() {
    assert_eq!(rendered("<loc>Jita</loc>"), "Jita");
  }

  #[test]
  fn it_tolerates_interleaved_loc_and_anchor_closes_with_a_stray_char() {
    let html = r##"<a href="http://x">https://pod.aaronmallen.de</loc>v</a>"##;
    let spans = parse_stored_body(html);
    assert_eq!(rendered(html), "https://pod.aaronmallen.dev");
    assert!(spans.iter().all(|s| s.link), "all text stays inside the link");
  }

  #[test]
  fn it_ignores_a_stray_closing_tag() {
    assert_eq!(rendered("hello</font> world"), "hello world");
  }

  #[test]
  fn it_renders_the_real_402644224_sample_without_dropping_text() {
    let html = concat!(
      r##"<font size="14" color="#bfffffff"><b>Bold Text</b><br><br><i>Italic Text</i><br><br>"##,
      r##"<u>Underline Text</u><br><br></font><font size="14" color="#ffff0000">Colored Text<br></font>"##,
      r##"<font size="24" color="#ffffffff">Diffrent Sized Text<br></font>"##,
      r##"<font size="14" color="#ffffe400"><loc><a href="http://pod.aaronmallen.dev">"##,
      r##"https://pod.aaronmallen.de</loc>v</a><br></font>"##,
      r##"<font size="14" color="#ffd98d00"><loc><a href="showinfo:1375//2124457086">Pod Use</loc>r</a><br>"##,
      r##"<loc><a href="showinfo:601">Ibi</loc>s</a><br></font>"##,
      r##"<font size="14" color="#bfffffff">normal text</font>"##,
    );
    let spans = parse_stored_body(html);
    let out = rendered(html);
    assert!(out.contains("Bold Text"));
    assert!(out.contains("Diffrent Sized Text"));
    assert!(out.contains("https://pod.aaronmallen.dev"));
    assert!(out.contains("Pod User"));
    assert!(out.contains("Ibis"));
    assert!(out.contains("normal text"));

    let bold = spans.iter().find(|s| s.text == "Bold Text").expect("bold");
    assert!(bold.bold && bold.size == 14.0);
    let big = spans.iter().find(|s| s.text.starts_with("Diffrent")).expect("big");
    assert_eq!(big.size, 24.0);
    let colored = spans.iter().find(|s| s.text.starts_with("Colored")).expect("colored");
    assert_eq!(colored.color, Some(iced::Color::from_rgb8(255, 0, 0)));
  }

  #[test]
  fn it_renders_the_real_sample_into_a_view_without_panicking() {
    use crate::store::model::{CharacterMail, CharacterMailBody};
    let html = r##"<font size="24" color="#ffffffff">Diffrent Sized Text<br></font><font size="14" color="#ffffe400"><loc><a href="http://x">https://pod.aaronmallen.de</loc>v</a></font>"##;
    let mail = MailRender {
      body: CharacterMailBody {
        body: html.to_owned(),
        character_id: 1,
        mail_id: 402_644_224,
      },
      header: CharacterMail {
        character_id: 1,
        mail_id: 402_644_224,
        ..Default::default()
      },
      label_ids: Vec::new(),
      recipients: Vec::new(),
      recipients_display: String::new(),
    };
    let _el: Element<'_, Message> = body_paragraphs(&mail);
  }

  mod pane {
    use super::*;
    use crate::store::{
      images,
      model::{CharacterMail, CharacterMailBody, CharacterMailRecipient},
    };

    fn render(from_id: i64, recipient_type: &str, recipients_display: &str, is_starred: bool) -> ReadingRender {
      ReadingRender {
        is_starred,
        labels: vec![MessageLabel {
          color: Some("#ff6600".to_owned()),
          name: "Fleet".to_owned(),
        }],
        mail: MailRender {
          body: CharacterMailBody {
            body: "<p>Form up at Jita.</p><p>Bring tackle.</p>".to_owned(),
            character_id: 42,
            mail_id: 7,
          },
          header: CharacterMail {
            character_id: 42,
            from_id,
            from_name: "Vex Voronova".to_owned(),
            is_read: true,
            mail_id: 7,
            subject: Some("CTA tonight".to_owned()),
            timestamp: "2026-06-01T10:00:00Z".to_owned(),
            ..Default::default()
          },
          label_ids: vec![8],
          recipients: vec![CharacterMailRecipient {
            character_id: 42,
            mail_id: 7,
            recipient_id: 42,
            recipient_name: "Vex Voronova".to_owned(),
            recipient_type: recipient_type.to_owned(),
          }],
          recipients_display: recipients_display.to_owned(),
        },
        sender_portrait: images::ImageState::Stale {
          id: 95_000_001,
          kind: images::ImageKind::CharacterPortrait,
        },
      }
    }

    #[test]
    fn it_renders_a_snoozed_system_message_addressed_to_me() {
      let render = render(0, "mailing_list", "  ", false);
      let _el: Element<'_, Message> = super::super::pane(Some(&render), true, false);
    }

    #[test]
    fn it_renders_an_opened_starred_mail() {
      let render = render(95_000_001, "character", "Vex Voronova", true);
      let _el: Element<'_, Message> = super::super::pane(Some(&render), false, true);
    }

    #[test]
    fn it_renders_the_empty_reading_pane() {
      let _el: Element<'_, Message> = super::super::pane(None, false, false);
    }
  }
}
