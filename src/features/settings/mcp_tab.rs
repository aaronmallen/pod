use iced::{
  Background, Border, Element, Length, Padding, Task,
  alignment::Vertical,
  widget::{Column, Row, Space, button, container, scrollable, svg, text, text_input},
};

use super::Outcome;
use crate::{
  config::Settings,
  ui::{
    components::{rule, toggle},
    style::{color, radius, spacing, typography},
  },
};

const BIND_ADDRESS: &str = "127.0.0.1";
const CARD_MAX_WIDTH: f32 = 660.0;
const DEFAULT_PORT: u16 = 7373;
const MASKED_TOKEN_LEN: usize = 48;
const MIN_PORT: u16 = 1024;
const PANEL_SIDE_PADDING: f32 = 36.0;
const STATUS_DOT_SIZE: f32 = 9.0;

static COPY_ICON: &[u8] = include_bytes!("../../../assets/images/icons/copy.svg");

const EFFECT_TOOLS: [Tool; 3] = [
  Tool {
    desc: "Agents may compose and send EVE-mail from your characters through ESI.",
    id: Perm::SendMail,
    title: "Allow agents to send mail",
  },
  Tool {
    desc: "Agents may permanently delete messages from your EVE-mail.",
    id: Perm::DeleteMail,
    title: "Allow agents to delete mail",
  },
  Tool {
    desc: "Agents may create, rename, delete, and reassign your mail labels.",
    id: Perm::ManageLabels,
    title: "Allow agents to manage labels",
  },
];

const SAFE_TOOLS: [Tool; 2] = [
  Tool {
    desc: "Let agents see characters, wallet, ledger, market, contracts, skills, industry & planner, \
      assets, mail, and prices. Read-only \u{2014} nothing is written and nothing leaves Pod.",
    id: Perm::Read,
    title: "Read tools",
  },
  Tool {
    desc: "Let agents assign budgets, build skill plans, and configure the industry planner. These \
      write only to Pod\u{2019}s local database \u{2014} zero EVE blast radius.",
    id: Perm::LocalWrite,
    title: "Local write tools",
  },
];

#[derive(Clone, Debug)]
pub enum Message {
  CopyConfig,
  CopyToken,
  EnabledToggled(bool),
  PermToggled(Perm, bool),
  PortEdited(String),
  PortSubmitted,
  ResetToken,
  ToggleTokenReveal,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Perm {
  DeleteMail,
  LocalWrite,
  ManageLabels,
  Read,
  SendMail,
}

impl Perm {
  fn is_on(self, settings: &Settings) -> bool {
    let perms = settings.mcp().perms();
    match self {
      Perm::DeleteMail => perms.delete_mail(),
      Perm::LocalWrite => perms.local_write(),
      Perm::ManageLabels => perms.manage_labels(),
      Perm::Read => perms.read(),
      Perm::SendMail => perms.send_mail(),
    }
  }

  fn set(self, settings: &mut Settings, value: bool) {
    let mut perms = *settings.mcp().perms();
    match self {
      Perm::DeleteMail => perms.set_delete_mail(value),
      Perm::LocalWrite => perms.set_local_write(value),
      Perm::ManageLabels => perms.set_manage_labels(value),
      Perm::Read => perms.set_read(value),
      Perm::SendMail => perms.set_send_mail(value),
    };
    settings.mcp_mut().set_perms(perms);
  }
}

#[derive(Debug, Default)]
pub struct State {
  port_draft: Option<String>,
  token_revealed: bool,
}

impl State {
  pub fn from_settings(settings: &Settings) -> Self {
    State {
      port_draft: Some(settings.mcp().port().to_string()),
      token_revealed: false,
    }
  }

  fn port_value(&self) -> &str {
    self.port_draft.as_deref().unwrap_or_default()
  }
}

#[derive(Clone, Copy, Debug)]
struct Tool {
  desc: &'static str,
  id: Perm,
  title: &'static str,
}

fn clamp_port(raw: &str) -> u16 {
  let digits: String = raw.chars().filter(char::is_ascii_digit).take(5).collect();
  match digits.parse::<u32>() {
    Ok(parsed) if parsed >= u32::from(MIN_PORT) => u16::try_from(parsed).unwrap_or(u16::MAX),
    _ => DEFAULT_PORT,
  }
}

fn config_snippet(port: u16, token: &str) -> String {
  format!(
    "{{\n  \"mcpServers\": {{\n    \"pod\": {{\n      \"url\": \"http://{BIND_ADDRESS}:{port}/mcp\",\n      \
      \"headers\": {{ \"Authorization\": \"Bearer {token}\" }}\n    }}\n  }}\n}}"
  )
}

fn effects_on(settings: &Settings) -> usize {
  EFFECT_TOOLS.iter().filter(|tool| tool.id.is_on(settings)).count()
}

pub fn badge(settings: &Settings) -> String {
  if *settings.mcp().enabled() {
    format!(":{}", settings.mcp().port())
  } else {
    "Off".to_owned()
  }
}

pub fn update(state: &mut State, message: Message, settings: &mut Settings) -> (Outcome, Task<Message>) {
  match message {
    Message::CopyConfig => {
      let token = settings.mcp_mut().token_or_generate();
      let snippet = config_snippet(*settings.mcp().port(), &token);
      (Outcome::McpChanged, iced::clipboard::write(snippet))
    }
    Message::CopyToken => {
      let token = settings.mcp_mut().token_or_generate();
      (Outcome::McpChanged, iced::clipboard::write(token))
    }
    Message::EnabledToggled(enabled) => {
      settings.mcp_mut().set_enabled(enabled);
      if enabled {
        settings.mcp_mut().token_or_generate();
      }
      (Outcome::McpChanged, Task::none())
    }
    Message::PermToggled(perm, value) => {
      perm.set(settings, value);
      (Outcome::McpChanged, Task::none())
    }
    Message::PortEdited(raw) => {
      let digits: String = raw.chars().filter(char::is_ascii_digit).take(5).collect();
      state.port_draft = Some(digits);
      (Outcome::None, Task::none())
    }
    Message::PortSubmitted => {
      let draft = state.port_value().to_owned();
      let port = clamp_port(&draft);
      settings.mcp_mut().set_port(port);
      state.port_draft = Some(port.to_string());
      (Outcome::McpChanged, Task::none())
    }
    Message::ResetToken => {
      settings.mcp_mut().set_token(String::new());
      settings.mcp_mut().token_or_generate();
      (Outcome::McpChanged, Task::none())
    }
    Message::ToggleTokenReveal => {
      state.token_revealed = !state.token_revealed;
      (Outcome::None, Task::none())
    }
  }
}

pub fn view<'a>(state: &'a State, settings: &'a Settings) -> Element<'a, Message> {
  let header = panel_header(settings);
  let body = panel_body(state, settings);

  Column::with_children(vec![header, body])
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn panel_header(settings: &Settings) -> Element<'_, Message> {
  let title = text("MCP Server")
    .font(typography::body::MEDIUM)
    .size(typography::size::LG)
    .style(typography::colored(color::text::PRIMARY));
  let blurb = text(
    "Run an embedded Model Context Protocol server so AI agents \u{2014} Claude Desktop, Claude Code \
      \u{2014} can automate Pod on your behalf. It stays off until you turn it on, binds to localhost \
      only, and exposes nothing without your token.",
  )
  .font(typography::body::REGULAR)
  .size(typography::size::MD)
  .style(typography::colored(color::text::secondary()));
  let identity = Column::with_children(vec![title.into(), blurb.into()])
    .spacing(spacing::UNIT)
    .width(Length::Fill);

  let row = Row::with_children(vec![identity.into(), server_badge(settings)])
    .align_y(Vertical::Bottom)
    .spacing(spacing::SPACE_4_5);

  let band = container(row).width(Length::Fill).padding(Padding {
    top: spacing::SPACE_6,
    right: PANEL_SIDE_PADDING,
    bottom: spacing::SPACE_3_5,
    left: PANEL_SIDE_PADDING,
  });

  Column::with_children(vec![band.into(), rule::horizontal()])
    .width(Length::Fill)
    .into()
}

fn panel_body<'a>(state: &'a State, settings: &'a Settings) -> Element<'a, Message> {
  let on = *settings.mcp().enabled();

  let server_head = section_head(
    "Server",
    "The master switch. Everything below is inert until the server is running.",
    if on { "Listening" } else { "Stopped" }.to_owned(),
    on,
  );
  let server_card = server_card(state, settings);
  let bind_note = bind_note();

  let auth_head = section_head(
    "Authentication",
    "Agents authenticate with a bearer token. Paste it into your agent\u{2019}s config \u{2014} treat \
      it like a password.",
    "Bearer".to_owned(),
    false,
  );
  let auth = gated(on, auth_section(state, settings));

  let perm_head = section_head(
    "Permissions",
    "What connected agents are allowed to do. A disabled tool returns a permission-denied error to the \
      agent.",
    perm_counter_label(settings),
    effects_on(settings) > 0,
  );
  let perms = gated(on, perm_section(settings));

  let connect_head = section_head(
    "Connect an agent",
    "Add Pod to your agent\u{2019}s MCP config, then restart the agent. It discovers Pod\u{2019}s tools \
      automatically.",
    "Config".to_owned(),
    false,
  );
  let connect = gated(on, connect_section(settings));

  let inner = container(
    Column::with_children(vec![
      server_head,
      server_card,
      bind_note,
      auth_head,
      auth,
      perm_head,
      perms,
      connect_head,
      connect,
    ])
    .spacing(spacing::SPACE_3_5)
    .width(Length::Fill),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::UNIT,
    right: PANEL_SIDE_PADDING,
    bottom: spacing::SPACE_6,
    left: PANEL_SIDE_PADDING,
  });

  scrollable(inner)
    .style(crate::ui::style::control::scrollbar)
    .width(Length::Fill)
    .height(Length::Fill)
    .into()
}

fn auth_section<'a>(state: &'a State, settings: &'a Settings) -> Element<'a, Message> {
  let token = settings.mcp().token();
  let display = if state.token_revealed {
    if token.is_empty() {
      "\u{2014}".to_owned()
    } else {
      token.clone()
    }
  } else {
    "\u{2022}".repeat(MASKED_TOKEN_LEN)
  };

  let head = container(
    Row::with_children(vec![
      text("Bearer token")
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(typography::colored(color::text::secondary()))
        .into(),
      Space::new().width(Length::Fill).into(),
      text("Read-only")
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(typography::colored(color::text::tertiary()))
        .into(),
    ])
    .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::SPACE_3,
    right: spacing::SPACE_3_5,
    bottom: spacing::SPACE_2_5,
    left: spacing::SPACE_3_5,
  });

  let value = text(display)
    .font(typography::mono::REGULAR)
    .size(typography::size::MD)
    .style(typography::colored(color::text::PRIMARY))
    .width(Length::Fill);
  let reveal = ghost_text_button(
    if state.token_revealed { "Hide" } else { "Show" },
    Message::ToggleTokenReveal,
  );
  let copy = primary_copy_button("Copy token", Message::CopyToken);

  let value_row = container(
    Row::with_children(vec![value.into(), reveal, copy])
      .align_y(Vertical::Center)
      .spacing(spacing::SPACE_3),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::SPACE_3,
    right: spacing::SPACE_3_5,
    bottom: spacing::SPACE_3,
    left: spacing::SPACE_3_5,
  });

  let field = container(
    Column::with_children(vec![head.into(), rule::horizontal_alpha(0.08), value_row.into()]).width(Length::Fill),
  )
  .max_width(CARD_MAX_WIDTH)
  .style(sunken_card_style);

  let reset = ghost_text_button("Reset token", Message::ResetToken);
  let reset_note = text(
    "Generates a new token and immediately invalidates the old one. Any connected agent stops working \
      until you re-paste the new token into its config.",
  )
  .font(typography::body::REGULAR)
  .size(typography::size::SM)
  .style(typography::colored(color::text::secondary()));
  let reset_row = container(
    Row::with_children(vec![reset, reset_note.into()])
      .align_y(Vertical::Center)
      .spacing(spacing::SPACE_3_5),
  )
  .max_width(CARD_MAX_WIDTH);

  Column::with_children(vec![field.into(), reset_row.into()])
    .spacing(spacing::SPACE_3_5)
    .width(Length::Fill)
    .into()
}

fn bind_note<'a>() -> Element<'a, Message> {
  let note = text(format!(
    "The bind address is fixed to {BIND_ADDRESS} \u{2014} localhost only, never reachable from another \
      machine. Only the port is configurable."
  ))
  .font(typography::body::REGULAR)
  .size(typography::size::SM)
  .style(typography::colored(color::text::secondary()));

  container(note).max_width(CARD_MAX_WIDTH).into()
}

fn connect_section(settings: &Settings) -> Element<'_, Message> {
  let snippet = config_snippet(*settings.mcp().port(), "<token>");

  let head = container(
    Row::with_children(vec![
      text("claude_desktop_config.json")
        .font(typography::mono::REGULAR)
        .size(typography::size::XS)
        .style(typography::colored(color::text::secondary()))
        .into(),
      Space::new().width(Length::Fill).into(),
      primary_copy_button("Copy config", Message::CopyConfig),
    ])
    .align_y(Vertical::Center),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::SPACE_2_5,
    right: spacing::SPACE_3,
    bottom: spacing::SPACE_2_5,
    left: spacing::SPACE_3_5,
  });

  let code = container(
    text(snippet)
      .font(typography::mono::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::PRIMARY)),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::SPACE_3_5,
    right: spacing::SPACE_4_5,
    bottom: spacing::SPACE_3_5,
    left: spacing::SPACE_4_5,
  });

  let card =
    container(Column::with_children(vec![head.into(), rule::horizontal_alpha(0.08), code.into()]).width(Length::Fill))
      .max_width(CARD_MAX_WIDTH)
      .style(sunken_card_style);

  let placeholder_note =
    text("<token> is filled in with your real bearer token when you hit Copy config \u{2014} paste and go.")
      .font(typography::body::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::secondary()));
  let log_note = text(
    "Everything an agent does is recorded to Pod\u{2019}s normal logs \u{2014} review them under Settings \
      \u{203a} Storage.",
  )
  .font(typography::body::REGULAR)
  .size(typography::size::SM)
  .style(typography::colored(color::text::tertiary()));

  Column::with_children(vec![
    card.into(),
    container(placeholder_note).max_width(CARD_MAX_WIDTH).into(),
    container(log_note).max_width(CARD_MAX_WIDTH).into(),
  ])
  .spacing(spacing::SPACE_2_5)
  .width(Length::Fill)
  .into()
}

fn gated(active: bool, content: Element<'_, Message>) -> Element<'_, Message> {
  if active {
    return content;
  }

  container(content)
    .width(Length::Fill)
    .style(|_| container::Style {
      background: Some(Background::Color(color::with_alpha(color::surface::SUNKEN, 0.4))),
      ..container::Style::default()
    })
    .into()
}

fn perm_counter_label(settings: &Settings) -> String {
  match effects_on(settings) {
    0 => "Local only".to_owned(),
    1 => "1 EVE effect on".to_owned(),
    count => format!("{count} EVE effects on"),
  }
}

fn perm_row(tool: Tool, on: bool) -> Element<'static, Message> {
  let heading = text(tool.title)
    .font(typography::body::MEDIUM)
    .size(typography::size::MD)
    .style(typography::colored(color::text::PRIMARY));
  let mut title_children: Vec<Element<'static, Message>> = vec![heading.into()];
  if matches!(tool.id, Perm::DeleteMail | Perm::ManageLabels | Perm::SendMail) {
    title_children.push(perm_tag("EVE"));
    title_children.push(perm_tag("WRITE"));
  }
  let title = Row::with_children(title_children)
    .align_y(Vertical::Center)
    .spacing(spacing::SPACE_2);

  let desc = text(tool.desc)
    .font(typography::body::REGULAR)
    .size(typography::size::SM)
    .style(typography::colored(color::text::secondary()));
  let identity = Column::with_children(vec![title.into(), desc.into()])
    .spacing(spacing::UNIT)
    .width(Length::Fill);

  let switch = toggle::toggle(on, Message::PermToggled(tool.id, !on));

  let row = Row::with_children(vec![identity.into(), switch])
    .align_y(Vertical::Center)
    .spacing(spacing::SPACE_6)
    .width(Length::Fill);

  container(row)
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_3,
      right: 0.0,
      bottom: spacing::SPACE_3,
      left: 0.0,
    })
    .into()
}

fn perm_section(settings: &Settings) -> Element<'_, Message> {
  let mut safe_rows: Vec<Element<'_, Message>> = vec![group_label("Safe \u{00b7} local only", color::status::ONLINE)];
  for tool in SAFE_TOOLS {
    safe_rows.push(perm_row(tool, tool.id.is_on(settings)));
  }
  let safe = Column::with_children(safe_rows).width(Length::Fill);

  let mut effect_rows: Vec<Element<'_, Message>> = vec![group_label("Real EVE effects", color::status::WARNING)];
  for tool in EFFECT_TOOLS {
    effect_rows.push(perm_row(tool, tool.id.is_on(settings)));
  }
  let warn_note = text(
    "These let an agent change things in EVE for real, through ESI. Leave a tool off and any agent call \
      to it is refused with a permission-denied error.",
  )
  .font(typography::body::REGULAR)
  .size(typography::size::SM)
  .style(typography::colored(color::text::secondary()));
  effect_rows.push(warn_note.into());
  let effects = container(Column::with_children(effect_rows).width(Length::Fill))
    .width(Length::Fill)
    .padding(Padding {
      top: spacing::SPACE_3,
      right: spacing::SPACE_4_5,
      bottom: spacing::SPACE_3_5,
      left: spacing::SPACE_4_5,
    })
    .style(|_| container::Style {
      background: Some(Background::Color(color::with_alpha(color::status::WARNING, 0.04))),
      border: Border {
        color: color::with_alpha(color::status::WARNING, 0.32),
        width: 1.0,
        radius: radius::CARD.into(),
      },
      ..container::Style::default()
    });

  Column::with_children(vec![safe.into(), effects.into()])
    .spacing(spacing::SPACE_6)
    .width(Length::Fill)
    .max_width(CARD_MAX_WIDTH)
    .into()
}

fn perm_tag(label: &'static str) -> Element<'static, Message> {
  container(
    text(label)
      .font(typography::mono::MEDIUM)
      .size(typography::size::XS)
      .style(typography::colored(color::status::WARNING)),
  )
  .padding(Padding {
    top: 1.0,
    right: spacing::SPACE_2 - 2.0,
    bottom: 1.0,
    left: spacing::SPACE_2 - 2.0,
  })
  .style(|_| container::Style {
    background: Some(Background::Color(color::with_alpha(color::status::WARNING, 0.07))),
    border: Border {
      color: color::with_alpha(color::status::WARNING, 0.32),
      width: 1.0,
      radius: radius::SUBTLE.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn group_label(label: &'static str, accent: iced::Color) -> Element<'static, Message> {
  let dot = container(Space::new())
    .width(Length::Fixed(6.0))
    .height(Length::Fixed(6.0))
    .style(move |_| container::Style {
      background: Some(Background::Color(accent)),
      border: Border {
        radius: radius::CONTROL.into(),
        ..Border::default()
      },
      ..container::Style::default()
    });
  let text_label = text(label)
    .font(typography::mono::MEDIUM)
    .size(typography::size::XS_PLUS)
    .style(typography::colored(accent));

  Row::with_children(vec![dot.into(), text_label.into()])
    .align_y(Vertical::Center)
    .spacing(spacing::SPACE_2_5)
    .into()
}

fn ghost_text_button(label: &'static str, message: Message) -> Element<'static, Message> {
  button(text(label).font(typography::body::MEDIUM).size(typography::size::MD))
    .padding(crate::ui::style::control::padding())
    .on_press(message)
    .style(crate::ui::style::control::ghost_button)
    .into()
}

fn primary_copy_button(label: &'static str, message: Message) -> Element<'static, Message> {
  let icon = svg(svg::Handle::from_memory(COPY_ICON))
    .width(Length::Fixed(14.0))
    .height(Length::Fixed(14.0))
    .style(|_, _| svg::Style {
      color: Some(color::surface::BASE),
    });
  let content = Row::with_children(vec![
    icon.into(),
    text(label)
      .font(typography::body::MEDIUM)
      .size(typography::size::MD)
      .into(),
  ])
  .align_y(Vertical::Center)
  .spacing(spacing::SPACE_2);

  button(content)
    .padding(crate::ui::style::control::padding())
    .on_press(message)
    .style(crate::ui::style::control::primary_button)
    .into()
}

fn section_head(label: &'static str, note: &'static str, chip: String, lit: bool) -> Element<'static, Message> {
  let micro = text(label)
    .font(typography::mono::MEDIUM)
    .size(typography::size::XS_PLUS)
    .style(typography::colored(color::accent::PLASMA));
  let detail = text(note)
    .font(typography::body::REGULAR)
    .size(typography::size::SM)
    .style(typography::colored(color::text::secondary()));
  let identity = Column::with_children(vec![micro.into(), detail.into()])
    .spacing(spacing::UNIT)
    .width(Length::Fill);

  let chip_color = if lit {
    color::accent::PLASMA
  } else {
    color::text::tertiary()
  };
  let chip = text(chip)
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(typography::colored(chip_color));

  let row = Row::with_children(vec![identity.into(), chip.into()])
    .align_y(Vertical::Bottom)
    .spacing(spacing::SPACE_3_5);

  let band = container(row).width(Length::Fill).padding(Padding {
    top: spacing::SPACE_6,
    right: 0.0,
    bottom: spacing::SPACE_3,
    left: 0.0,
  });

  Column::with_children(vec![band.into(), rule::horizontal_alpha(0.18)])
    .width(Length::Fill)
    .into()
}

fn server_badge(settings: &Settings) -> Element<'_, Message> {
  let on = *settings.mcp().enabled();
  let accent = if on {
    color::status::ONLINE
  } else {
    color::text::secondary()
  };
  let dot = container(Space::new())
    .width(Length::Fixed(7.0))
    .height(Length::Fixed(7.0))
    .style(move |_| container::Style {
      background: Some(Background::Color(accent)),
      border: Border {
        radius: radius::CONTROL.into(),
        ..Border::default()
      },
      ..container::Style::default()
    });
  let label_text = if on {
    format!("On \u{00b7} :{}", settings.mcp().port())
  } else {
    "Off".to_owned()
  };
  let label = text(label_text)
    .font(typography::mono::REGULAR)
    .size(typography::size::XS_PLUS)
    .style(typography::colored(accent));

  container(
    Row::with_children(vec![dot.into(), label.into()])
      .align_y(Vertical::Center)
      .spacing(spacing::SPACE_2),
  )
  .padding(Padding {
    top: spacing::SPACE_2 - 2.0,
    right: spacing::SPACE_3,
    bottom: spacing::SPACE_2 - 2.0,
    left: spacing::SPACE_3,
  })
  .style(move |_| container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    border: Border {
      color: color::with_alpha(accent, 0.4),
      width: 1.0,
      radius: radius::CONTROL.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn server_card<'a>(state: &'a State, settings: &'a Settings) -> Element<'a, Message> {
  let on = *settings.mcp().enabled();

  let heading = text("Enable MCP server")
    .font(typography::body::MEDIUM)
    .size(typography::size::LG)
    .style(typography::colored(color::text::PRIMARY));
  let blurb = text(
    "Opens a localhost port for connected agents. Turn this off any time to instantly cut every \
      agent\u{2019}s access.",
  )
  .font(typography::body::REGULAR)
  .size(typography::size::SM)
  .style(typography::colored(color::text::secondary()));
  let identity = Column::with_children(vec![heading.into(), blurb.into()])
    .spacing(spacing::SPACE_2)
    .width(Length::Fill);
  let enable_row = container(
    Row::with_children(vec![identity.into(), toggle::toggle(on, Message::EnabledToggled(!on))])
      .align_y(Vertical::Center)
      .spacing(spacing::SPACE_6)
      .width(Length::Fill),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::SPACE_4_5,
    right: spacing::SPACE_4_5,
    bottom: spacing::SPACE_4_5,
    left: spacing::SPACE_4_5,
  });

  let status_dot = container(Space::new())
    .width(Length::Fixed(STATUS_DOT_SIZE))
    .height(Length::Fixed(STATUS_DOT_SIZE))
    .style(move |_| container::Style {
      background: Some(Background::Color(if on {
        color::status::ONLINE
      } else {
        color::text::tertiary()
      })),
      border: Border {
        radius: radius::CONTROL.into(),
        ..Border::default()
      },
      ..container::Style::default()
    });
  let status_caption = text("Status")
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(typography::colored(color::text::tertiary()));
  let status_value = if on {
    text(format!(
      "Running \u{00b7} http://{BIND_ADDRESS}:{}/mcp",
      settings.mcp().port()
    ))
    .font(typography::mono::REGULAR)
    .size(typography::size::MD)
    .style(typography::colored(color::accent::PLASMA))
  } else {
    text("Stopped \u{00b7} no port open")
      .font(typography::mono::REGULAR)
      .size(typography::size::MD)
      .style(typography::colored(color::text::secondary()))
  };
  let status = container(
    Row::with_children(vec![
      status_dot.into(),
      Column::with_children(vec![status_caption.into(), status_value.into()])
        .spacing(spacing::UNIT)
        .width(Length::Fill)
        .into(),
    ])
    .align_y(Vertical::Center)
    .spacing(spacing::SPACE_3),
  )
  .width(Length::Fill)
  .padding(Padding {
    top: spacing::SPACE_3_5,
    right: spacing::SPACE_4_5,
    bottom: spacing::SPACE_3_5,
    left: spacing::SPACE_4_5,
  });

  let port_caption = text("Port")
    .font(typography::mono::REGULAR)
    .size(typography::size::XS)
    .style(typography::colored(color::text::tertiary()));
  let port_field = Row::with_children(vec![
    text(format!("{BIND_ADDRESS} :"))
      .font(typography::mono::REGULAR)
      .size(typography::size::SM)
      .style(typography::colored(color::text::tertiary()))
      .into(),
    text_input("7373", state.port_value())
      .font(typography::mono::MEDIUM)
      .size(typography::size::MD)
      .padding(0)
      .width(Length::Fixed(64.0))
      .on_input(Message::PortEdited)
      .on_submit(Message::PortSubmitted)
      .style(port_input_style)
      .into(),
  ])
  .align_y(Vertical::Center)
  .spacing(spacing::SPACE_2);
  let port = container(Column::with_children(vec![port_caption.into(), port_field.into()]).spacing(spacing::SPACE_2))
    .width(Length::Fixed(220.0))
    .padding(Padding {
      top: spacing::SPACE_3_5,
      right: spacing::SPACE_4_5,
      bottom: spacing::SPACE_3_5,
      left: spacing::SPACE_4_5,
    });

  let status_row = Row::with_children(vec![status.into(), rule::vertical_fill(0.08), port.into()])
    .align_y(Vertical::Center)
    .width(Length::Fill);

  let edge = if on {
    color::with_alpha(color::accent::PLASMA, 0.45)
  } else {
    color::rule_strong()
  };

  container(
    Column::with_children(vec![enable_row.into(), rule::horizontal_alpha(0.08), status_row.into()]).width(Length::Fill),
  )
  .max_width(CARD_MAX_WIDTH)
  .style(move |_| container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    border: Border {
      color: edge,
      width: 1.0,
      radius: radius::CARD.into(),
    },
    ..container::Style::default()
  })
  .into()
}

fn port_input_style(_theme: &iced::Theme, _status: text_input::Status) -> text_input::Style {
  text_input::Style {
    background: Background::Color(iced::Color::TRANSPARENT),
    border: Border::default(),
    icon: color::text::secondary(),
    placeholder: color::text::tertiary(),
    value: color::text::PRIMARY,
    selection: color::with_alpha(color::accent::PLASMA, 0.4),
  }
}

fn sunken_card_style(_theme: &iced::Theme) -> container::Style {
  container::Style {
    background: Some(Background::Color(color::surface::SUNKEN)),
    border: Border {
      color: color::rule_strong(),
      width: 1.0,
      radius: radius::CARD.into(),
    },
    ..container::Style::default()
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn with_effect(settings: &mut Settings, perm: Perm) {
    perm.set(settings, true);
  }

  mod badge {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_reports_off_when_the_server_is_disabled() {
      assert_eq!(badge(&Settings::default()), "Off");
    }

    #[test]
    fn it_reports_the_bound_port_when_the_server_is_enabled() {
      let mut settings = Settings::default();
      settings.mcp_mut().set_enabled(true);
      settings.mcp_mut().set_port(8080);

      assert_eq!(badge(&settings), ":8080");
    }
  }

  mod clamp_port {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_clamps_a_value_above_the_tcp_ceiling() {
      assert_eq!(clamp_port("99999"), u16::MAX);
    }

    #[test]
    fn it_falls_back_to_the_default_for_a_blank_draft() {
      assert_eq!(clamp_port(""), DEFAULT_PORT);
    }

    #[test]
    fn it_falls_back_to_the_default_for_a_privileged_port() {
      assert_eq!(clamp_port("80"), DEFAULT_PORT);
    }

    #[test]
    fn it_keeps_a_valid_port() {
      assert_eq!(clamp_port("7373"), 7373);
    }

    #[test]
    fn it_strips_non_digits_before_parsing() {
      assert_eq!(clamp_port("7a3b7c3"), 7373);
    }
  }

  mod config_snippet {
    use super::*;

    #[test]
    fn it_embeds_the_port_and_token() {
      let snippet = config_snippet(7373, "pod_mcp_abc");

      assert!(snippet.contains("http://127.0.0.1:7373/mcp"));
      assert!(snippet.contains("Bearer pod_mcp_abc"));
    }
  }

  mod effects_on {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_counts_only_the_enabled_eve_effect_perms() {
      let mut settings = Settings::default();
      with_effect(&mut settings, Perm::SendMail);
      with_effect(&mut settings, Perm::ManageLabels);

      assert_eq!(effects_on(&settings), 2);
    }

    #[test]
    fn it_ignores_the_default_safe_perms() {
      assert_eq!(effects_on(&Settings::default()), 0);
    }
  }

  mod perm_counter_label {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_pluralizes_multiple_effects() {
      let mut settings = Settings::default();
      with_effect(&mut settings, Perm::SendMail);
      with_effect(&mut settings, Perm::DeleteMail);

      assert_eq!(perm_counter_label(&settings), "2 EVE effects on");
    }

    #[test]
    fn it_reads_local_only_with_no_effects() {
      assert_eq!(perm_counter_label(&Settings::default()), "Local only");
    }

    #[test]
    fn it_uses_the_singular_for_one_effect() {
      let mut settings = Settings::default();
      with_effect(&mut settings, Perm::SendMail);

      assert_eq!(perm_counter_label(&settings), "1 EVE effect on");
    }
  }

  mod update {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn enabling_the_server_mints_a_token() {
      let mut state = State::default();
      let mut settings = Settings::default();

      let (outcome, _task) = update(&mut state, Message::EnabledToggled(true), &mut settings);

      assert_eq!(outcome, Outcome::McpChanged);
      assert!(*settings.mcp().enabled());
      assert!(!settings.mcp().token().is_empty());
    }

    #[test]
    fn resetting_the_token_replaces_it() {
      let mut state = State::default();
      let mut settings = Settings::default();
      settings.mcp_mut().set_token("pod_mcp_old".to_owned());

      let (outcome, _task) = update(&mut state, Message::ResetToken, &mut settings);

      assert_eq!(outcome, Outcome::McpChanged);
      assert_ne!(settings.mcp().token(), "pod_mcp_old");
      assert!(!settings.mcp().token().is_empty());
    }

    #[test]
    fn submitting_an_invalid_port_clamps_to_the_default() {
      let mut state = State::default();
      let mut settings = Settings::default();
      state.port_draft = Some("12".to_owned());

      let (outcome, _task) = update(&mut state, Message::PortSubmitted, &mut settings);

      assert_eq!(outcome, Outcome::McpChanged);
      assert_eq!(*settings.mcp().port(), DEFAULT_PORT);
    }

    #[test]
    fn toggling_a_perm_writes_through_to_the_config() {
      let mut state = State::default();
      let mut settings = Settings::default();

      let (outcome, _task) = update(&mut state, Message::PermToggled(Perm::SendMail, true), &mut settings);

      assert_eq!(outcome, Outcome::McpChanged);
      assert!(settings.mcp().perms().send_mail());
    }

    #[test]
    fn typing_a_port_strips_non_digits_without_persisting() {
      let mut state = State::default();
      let mut settings = Settings::default();

      let (outcome, _task) = update(&mut state, Message::PortEdited("8a0b8c0".to_owned()), &mut settings);

      assert_eq!(outcome, Outcome::None);
      assert_eq!(state.port_draft.as_deref(), Some("8080"));
      assert_eq!(*settings.mcp().port(), DEFAULT_PORT);
    }
  }

  mod view {
    use super::*;

    #[test]
    fn it_renders_the_panel_when_the_server_is_off() {
      let settings = Settings::default();
      let state = State::from_settings(&settings);

      let _el: Element<'_, Message> = view(&state, &settings);
    }

    #[test]
    fn it_renders_the_panel_when_the_server_is_on() {
      let mut settings = Settings::default();
      settings.mcp_mut().set_enabled(true);
      let state = State::from_settings(&settings);

      let _el: Element<'_, Message> = view(&state, &settings);
    }
  }
}
