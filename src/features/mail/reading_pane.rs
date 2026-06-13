use iced::{
  Background, Border, Element, Length, Padding,
  alignment::Vertical,
  widget::{Column, Row, Space, button, container, scrollable, text},
};

use super::{Message, ReadingRender, loaders::MessageLabel};
use crate::{
  store::model::character_mail_view::MailRender,
  ui::{
    components::{avatar::Avatar, chip::label_chip, empty_state::empty_state as shared_empty_state, icon::Icon, rule},
    style::{color, radius, spacing, typography},
  },
};

const TOOLBAR_SIDE_PADDING: f32 = 24.0;
const BODY_MAX_WIDTH: f32 = 720.0;

pub(super) fn pane(render: Option<&ReadingRender>, is_snoozed: bool) -> Element<'_, Message> {
  let body: Element<'_, Message> = match render {
    Some(render) => opened(render, is_snoozed),
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
  shared_empty_state("Select a message").render()
}

fn opened(render: &ReadingRender, is_snoozed: bool) -> Element<'_, Message> {
  Column::with_children(vec![toolbar(render, is_snoozed), scroll_body(render)])
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn toolbar(render: &ReadingRender, is_snoozed: bool) -> Element<'_, Message> {
  let mail_id = render.mail.header.mail_id();

  let star_tone = if render.is_starred {
    color::accent::PLASMA
  } else {
    color::text::secondary()
  };
  let snooze_tone = if is_snoozed {
    color::accent::PLASMA
  } else {
    color::text::secondary()
  };

  let mut row = Row::new().align_y(Vertical::Center);
  row = row.push(toolbar_button(
    Icon::reply(),
    "Reply",
    Message::Reply(mail_id),
    false,
    false,
  ));
  row = row.push(toolbar_button(
    Icon::reply_all(),
    "Reply all",
    Message::ReplyAll(mail_id),
    false,
    false,
  ));
  row = row.push(toolbar_button(
    Icon::forward(),
    "Forward",
    Message::Forward(mail_id),
    false,
    false,
  ));
  row = row.push(toolbar_button(
    Icon::tag(),
    "Label",
    Message::LabelPickerOpened(mail_id),
    !render.labels.is_empty(),
    false,
  ));
  row = row.push(toolbar_divider());
  row = row.push(toolbar_button(
    Icon::star().color(star_tone),
    if render.is_starred { "Starred" } else { "Star" },
    Message::ToggleStar(mail_id),
    render.is_starred,
    false,
  ));
  row = row.push(toolbar_button(
    Icon::snooze().color(snooze_tone),
    if is_snoozed { "Snoozed" } else { "Snooze" },
    Message::SnoozeMenuToggled,
    is_snoozed,
    false,
  ));
  row = row.push(toolbar_button(
    Icon::pin(),
    "Pin",
    Message::TogglePin(mail_id),
    false,
    false,
  ));
  row = row.push(toolbar_button(
    Icon::archive(),
    "Archive",
    Message::Archive(mail_id),
    false,
    false,
  ));
  row = row.push(toolbar_button(
    Icon::trash(),
    "Delete",
    Message::Trash(mail_id),
    false,
    true,
  ));
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

fn toolbar_button<'a>(icon: Icon, label: &str, message: Message, active: bool, danger: bool) -> Element<'a, Message> {
  let tone = if active {
    color::accent::PLASMA
  } else if danger {
    color::status::DANGER
  } else {
    color::text::secondary()
  };

  let content = Row::with_children(vec![
    icon.size(14.0).render::<Message>(),
    text(label.to_owned())
      .size(typography::size::MD - 1.0)
      .font(typography::body::MEDIUM)
      .style(move |_| text::Style {
        color: Some(tone),
      })
      .into(),
  ])
  .spacing(spacing::UNIT + 2.0)
  .align_y(Vertical::Center);

  button(content)
    .padding(Padding {
      top: spacing::UNIT + 2.0,
      bottom: spacing::UNIT + 2.0,
      left: spacing::SPACE_2_5,
      right: spacing::SPACE_2_5,
    })
    .on_press(message)
    .style(|_, status| toolbar_button_style(status))
    .into()
}

fn toolbar_button_style(status: button::Status) -> button::Style {
  let hovered = matches!(status, button::Status::Hovered | button::Status::Pressed);
  button::Style {
    background: hovered.then(|| Background::Color(color::with_alpha(color::text::PRIMARY, 0.06))),
    border: Border {
      radius: 6.0.into(),
      ..Border::default()
    },
    text_color: color::text::PRIMARY,
    ..button::Style::default()
  }
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
    .unwrap_or_else(|| "(no subject)".to_owned());

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
      text("to ")
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
        text(" · System message")
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
    "me".to_owned()
  } else {
    mail.recipients_display.clone()
  }
}

const SENDER_AVATAR_SIZE: f32 = 44.0;

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

fn body_paragraphs(mail: &MailRender) -> Element<'_, Message> {
  let paragraphs = parse_stored_body(mail.body.body());

  if paragraphs.is_empty() {
    return text("(no content)")
      .size(typography::size::MD)
      .style(|_| text::Style {
        color: Some(color::text::secondary()),
      })
      .into();
  }

  let mut column = Column::new()
    .spacing(spacing::SPACE_3 + spacing::SPACE_2)
    .width(Length::Fill);
  for paragraph in paragraphs {
    column = column.push(text(paragraph).size(typography::size::MD).style(|_| text::Style {
      color: Some(color::with_alpha(color::text::PRIMARY, 0.88)),
    }));
  }
  column.into()
}

fn parse_stored_body(html: &str) -> Vec<String> {
  let mut text = String::with_capacity(html.len());
  let bytes = html.as_bytes();
  let mut i = 0;
  while i < bytes.len() {
    if bytes[i] == b'<' {
      let start = i;
      while i < bytes.len() && bytes[i] != b'>' {
        i += 1;
      }
      let tag = html[start..(i.min(bytes.len()))].to_ascii_lowercase();
      if tag.starts_with("<br") || tag.starts_with("</p") || tag.starts_with("<p") || tag.starts_with("</div") {
        text.push('\n');
      }
      i += 1;
    } else {
      text.push(html[i..].chars().next().unwrap_or('\0'));
      i += html[i..].chars().next().map_or(1, char::len_utf8);
    }
  }

  text
    .split('\n')
    .map(|line| line.split_whitespace().collect::<Vec<_>>().join(" "))
    .filter(|line| !line.is_empty())
    .collect()
}

#[cfg(test)]
mod tests {
  use pretty_assertions::assert_eq;

  use super::*;

  #[test]
  fn it_splits_a_stored_html_body_into_trimmed_paragraphs() {
    let html = "<p>First line.</p><p>Second   line   here.</p>";
    assert_eq!(
      parse_stored_body(html),
      vec!["First line.".to_owned(), "Second line here.".to_owned()]
    );
  }

  #[test]
  fn it_treats_br_tags_as_paragraph_breaks_and_drops_blank_runs() {
    let html = "Alpha<br><br>Beta<br>";
    assert_eq!(parse_stored_body(html), vec!["Alpha".to_owned(), "Beta".to_owned()]);
  }

  #[test]
  fn it_strips_inline_tags_without_breaking_a_paragraph() {
    let html = "Form up at <b>Jita</b> by <i>19:15</i>.";
    assert_eq!(parse_stored_body(html), vec!["Form up at Jita by 19:15.".to_owned()]);
  }

  #[test]
  fn it_yields_no_paragraphs_for_an_empty_body() {
    assert!(parse_stored_body("").is_empty());
    assert!(parse_stored_body("<br><br>").is_empty());
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
    fn it_renders_an_opened_starred_mail() {
      let render = render(95_000_001, "character", "Vex Voronova", true);
      let _el: Element<'_, Message> = super::super::pane(Some(&render), false);
    }

    #[test]
    fn it_renders_a_snoozed_system_message_addressed_to_me() {
      // A zero sender id flags a system message, and a blank recipient display resolves to "me".
      let render = render(0, "mailing_list", "  ", false);
      let _el: Element<'_, Message> = super::super::pane(Some(&render), true);
    }

    #[test]
    fn it_renders_the_empty_reading_pane() {
      let _el: Element<'_, Message> = super::super::pane(None, false);
    }
  }
}
