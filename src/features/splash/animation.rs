use iced::{
  Element, Length,
  widget::{Stack, svg},
};

use crate::ui::style::color;

pub const HEIGHT: f32 = 200.0;
pub const UPDATE_HEIGHT: f32 = 120.0;

pub fn logo<'a, M>(rotation: f32, pulse: f32, expand: f32, height: f32) -> Element<'a, M>
where
  M: 'a,
{
  let letterforms = svg(letterforms_handle(rotation, expand))
    .width(Length::Fill)
    .height(Length::Fixed(height))
    .style(|_, _| svg::Style {
      color: Some(color::text::PRIMARY),
    });

  let dot = svg(dot_handle(pulse))
    .width(Length::Fill)
    .height(Length::Fixed(height))
    .style(|_, _| svg::Style {
      color: Some(color::accent()),
    });

  Stack::with_children(vec![letterforms.into(), dot.into()]).into()
}

fn dot_handle(pulse: f32) -> svg::Handle {
  let r = 10.0_f32 + 4.0 * pulse.sin();

  let content = format!(
    r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 -30 600 280">
<circle cx="300" cy="110" r="{r:.2}" fill="currentColor"/>
</svg>"##
  );

  svg::Handle::from_memory(content.into_bytes())
}

fn ease_out_cubic(t: f32) -> f32 {
  1.0 - (1.0 - t).powi(3)
}

fn letterforms_handle(rotation: f32, expand: f32) -> svg::Handle {
  let t = ease_out_cubic(expand.clamp(0.0, 1.0));

  let o_dx = 200.0_f32;
  let p_dx = 200.0 - t * 200.0;
  let d_dx = 200.0 + t * 200.0;
  let spin = rotation * (1.0 - t);

  let content = format!(
    r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 -30 600 280">
<g transform="rotate({spin:.2}, 300, 110)">
<g transform="translate({p_dx:.2}, 10)">
<circle cx="100" cy="100" r="58" fill="none" stroke="currentColor" stroke-width="18"/>
<line x1="42" y1="100" x2="42" y2="192" stroke="currentColor" stroke-width="18" stroke-linecap="round"/>
</g>
<g transform="translate({o_dx:.2}, 10)">
<circle cx="100" cy="100" r="58" fill="none" stroke="currentColor" stroke-width="18"/>
</g>
<g transform="translate({d_dx:.2}, 10)">
<circle cx="100" cy="100" r="58" fill="none" stroke="currentColor" stroke-width="18"/>
<line x1="158" y1="8" x2="158" y2="100" stroke="currentColor" stroke-width="18" stroke-linecap="round"/>
</g>
</g>
</svg>"##
  );

  svg::Handle::from_memory(content.into_bytes())
}
