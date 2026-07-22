use iced::{
  Element, Length,
  widget::{Space, Stack},
};

/// Forces a clip layer bounded to `width`×`height` so `ContentFit::Cover` images crop to that box on every renderer.
///
/// A plain `container.clip(true)` only narrows the child viewport without opening a new layer, which is enough for
/// iced_wgpu (per-image scissor). But iced_tiny_skia 0.14.0's `engine::draw_image` ignores each image's stored
/// `clip_bounds` and clips images only to the enclosing layer's bounds — so Cover overdraw is never cropped on the
/// software path (the AppImage Wayland fallback). Placing `content` as a non-base `Stack` child (index > 0) with
/// `clip(true)` causes iced to call `renderer.with_layer` for it, bounded to the Stack's box; the empty `Space` at
/// index 0 defines that box.
pub fn clip_layer<'a, M>(content: impl Into<Element<'a, M>>, width: Length, height: Length) -> Element<'a, M>
where
  M: 'a,
{
  Stack::with_children(vec![Space::new().width(width).height(height).into(), content.into()])
    .width(width)
    .height(height)
    .clip(true)
    .into()
}

#[cfg(test)]
mod tests {
  use super::*;

  mod clip_layer {
    use iced::{ContentFit, widget::image};

    use super::*;

    #[test]
    fn it_wraps_a_cover_image_in_a_clip_layer() {
      let img = image(image::Handle::from_path("/tmp/portrait.png"))
        .width(Length::Fill)
        .height(Length::Fill)
        .content_fit(ContentFit::Cover);

      let _el: Element<'_, ()> = clip_layer(img, Length::Fill, Length::Fixed(140.0));
    }
  }
}
