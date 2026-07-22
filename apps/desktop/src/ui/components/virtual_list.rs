//! A shared windowing helper for long, scrollable lists.
//!
//! iced 0.14 ships no virtualized-list widget, so every infinite-scroll surface
//! that hand-builds a `scrollable(Column::with_children(rows))` pays a layout and
//! draw cost proportional to the number of *loaded* rows, not the number of
//! *visible* rows. [`VirtualList`] materializes only the rows in (and just
//! around) the viewport, padding the gap above and below with [`Space`] spacers
//! so the scrollbar geometry is preserved.
//!
//! The helper is deliberately structure-agnostic: a surface flattens whatever it
//! displays (expandable container trees, day-bucket section headers, multi-column
//! card grids) into a single flat index space, and supplies a per-index renderer.
//! That keeps the windowing math in one place while letting each surface keep its
//! own row widgets.
//!
//! # Estimated heights
//!
//! Because the wrap/no-truncation table fidelity makes row heights content-driven
//! (a name cell may be one or two lines), there is no exact offset-to-row mapping.
//! The window is therefore computed from a per-surface *nominal* row height plus a
//! generous [`overscan`](VirtualListConfig::overscan) margin. The estimation only
//! affects scrollbar-thumb precision, never which rows are reachable: overscan
//! absorbs the small one-vs-two-line variance so no visible gap can open.
//!
//! # Usage
//!
//! ```ignore
//! use crate::ui::components::virtual_list::{VirtualList, VirtualListConfig};
//!
//! VirtualList::new(
//!   VirtualListConfig::new(rows.len(), est_row_height)
//!     .viewport_height(viewport.height)
//!     .scroll_offset(state.scroll_offset()),
//!   |index| render_row(&rows[index]),
//! )
//! .view()
//! ```
//!
//! For grid surfaces (e.g. the abyssals card grid) set
//! [`items_per_row`](VirtualListConfig::items_per_row); the helper then windows by
//! *row of cards* and the renderer is asked to build one row at a time.

use iced::{
  Element, Length,
  widget::{Column, Space, responsive},
};

pub const DEFAULT_OVERSCAN: usize = 8;

pub struct VirtualList<'a, Message, F> {
  config: VirtualListConfig,
  render_row: F,
  spacing: f32,
  _marker: std::marker::PhantomData<&'a Message>,
}

impl<'a, Message, F> VirtualList<'a, Message, F>
where
  Message: 'a,
  F: Fn(usize) -> Element<'a, Message>,
{
  pub fn new(config: VirtualListConfig, render_row: F) -> Self {
    Self {
      config,
      render_row,
      spacing: 0.0,
      _marker: std::marker::PhantomData,
    }
  }

  pub fn spacing(mut self, spacing: f32) -> Self {
    self.spacing = spacing;
    self
  }

  pub fn view(self) -> Element<'a, Message> {
    let window = self.config.window();
    let mut children: Vec<Element<'a, Message>> = Vec::with_capacity(window.len() + 2);

    if window.leading > 0.0 {
      children.push(Space::new().height(Length::Fixed(window.leading)).into());
    }
    for row in window.first_row..window.end_row {
      children.push((self.render_row)(row));
    }
    if window.trailing > 0.0 {
      children.push(Space::new().height(Length::Fixed(window.trailing)).into());
    }

    Column::with_children(children)
      .spacing(self.spacing)
      .width(Length::Fill)
      .into()
  }
}

#[derive(Clone, Copy, Debug)]
pub struct VirtualListConfig {
  estimated_row_height: f32,
  items_per_row: usize,
  overscan: usize,
  scroll_offset: f32,
  total_items: usize,
  viewport_height: f32,
}

impl VirtualListConfig {
  pub fn new(total_items: usize, estimated_row_height: f32) -> Self {
    Self {
      estimated_row_height: estimated_row_height.max(1.0),
      items_per_row: 1,
      overscan: DEFAULT_OVERSCAN,
      scroll_offset: 0.0,
      total_items,
      viewport_height: 0.0,
    }
  }

  pub fn items_per_row(mut self, items_per_row: usize) -> Self {
    self.items_per_row = items_per_row.max(1);
    self
  }

  #[cfg(test)]
  pub fn overscan(mut self, overscan: usize) -> Self {
    self.overscan = overscan;
    self
  }

  pub fn scroll_offset(mut self, offset: f32) -> Self {
    self.scroll_offset = offset.max(0.0);
    self
  }

  pub fn viewport_height(mut self, height: f32) -> Self {
    self.viewport_height = height.max(0.0);
    self
  }

  pub fn content_height(&self) -> f32 {
    self.total_rows() as f32 * self.estimated_row_height
  }

  /// The largest scroll offset the content can hold for the current viewport.
  ///
  /// Clamping a stored offset to this keeps a windowed list from rendering past
  /// its end when the row set shrinks under it (e.g. rows leave an active
  /// filter), which would otherwise snap the view to the top.
  pub fn max_scroll_offset(&self) -> f32 {
    (self.content_height() - self.viewport_height).max(0.0)
  }

  pub fn window(&self) -> WindowRange {
    let total_rows = self.total_rows();
    if total_rows == 0 {
      return WindowRange {
        end_row: 0,
        first_row: 0,
        leading: 0.0,
        trailing: 0.0,
      };
    }

    let row_height = self.estimated_row_height;
    let first_visible = (self.scroll_offset / row_height).floor() as usize;
    let raw_visible = (self.viewport_height / row_height).ceil();
    let visible_rows = if self.viewport_height.is_finite()
      && self.viewport_height > 0.0
      && raw_visible.is_finite()
      && raw_visible <= usize::MAX as f32
    {
      (raw_visible as usize).saturating_add(1)
    } else {
      self.overscan.max(1)
    };

    let first_row = first_visible.saturating_sub(self.overscan).min(total_rows);
    let end_row = first_visible
      .saturating_add(visible_rows)
      .saturating_add(self.overscan)
      .min(total_rows);

    let leading = first_row as f32 * row_height;
    let trailing = total_rows.saturating_sub(end_row) as f32 * row_height;

    WindowRange {
      end_row,
      first_row,
      leading,
      trailing,
    }
  }

  fn total_rows(&self) -> usize {
    self.total_items.div_ceil(self.items_per_row)
  }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowRange {
  pub end_row: usize,
  pub first_row: usize,
  pub leading: f32,
  pub trailing: f32,
}

impl WindowRange {
  #[cfg(test)]
  pub fn is_empty(&self) -> bool {
    self.first_row >= self.end_row
  }

  pub fn len(&self) -> usize {
    self.end_row.saturating_sub(self.first_row)
  }
}

pub fn responsive_window<'a, Message, F>(build: F) -> Element<'a, Message>
where
  Message: 'a,
  F: Fn(f32) -> Element<'a, Message> + 'a,
{
  responsive(move |size| build(size.height)).into()
}

#[cfg(test)]
mod tests {
  use super::*;

  mod max_scroll_offset {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_is_the_content_height_minus_the_viewport() {
      let config = VirtualListConfig::new(100, 40.0).viewport_height(400.0);

      assert_eq!(config.max_scroll_offset(), 100.0 * 40.0 - 400.0);
    }

    #[test]
    fn it_is_zero_when_the_content_fits_in_the_viewport() {
      let config = VirtualListConfig::new(3, 40.0).viewport_height(400.0);

      assert_eq!(config.max_scroll_offset(), 0.0);
    }

    #[test]
    fn it_clamps_a_stale_offset_so_a_shrunk_list_holds_position() {
      let stale = 4_200.0_f32;
      let shrunk = VirtualListConfig::new(3, 40.0).viewport_height(400.0);

      assert_eq!(stale.min(shrunk.max_scroll_offset()), 0.0);
    }
  }

  mod view {
    use super::*;

    #[test]
    fn it_renders_a_windowed_column() {
      let config = VirtualListConfig::new(1_000, 40.0)
        .viewport_height(400.0)
        .scroll_offset(2_000.0);
      let _el: Element<'_, ()> = VirtualList::new(config, |index| iced::widget::text(format!("row {index}")).into())
        .spacing(0.0)
        .view();
    }

    #[test]
    fn it_renders_through_the_responsive_wrapper() {
      let _el: Element<'_, ()> = responsive_window(|height| {
        VirtualList::new(VirtualListConfig::new(50, 40.0).viewport_height(height), |index| {
          iced::widget::text(format!("row {index}")).into()
        })
        .view()
      });
    }
  }

  mod window {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_centers_the_window_on_the_scroll_offset() {
      let window = VirtualListConfig::new(1_000, 40.0)
        .viewport_height(400.0)
        .scroll_offset(4_000.0)
        .overscan(8)
        .window();

      assert_eq!(window.first_row, 92);
      assert_eq!(window.end_row, 119);
      assert_eq!(window.leading, 92.0 * 40.0);
      assert_eq!(window.trailing, (1_000 - 119) as f32 * 40.0);
    }

    #[test]
    fn it_clamps_the_window_to_the_end_of_the_list() {
      let window = VirtualListConfig::new(100, 40.0)
        .viewport_height(400.0)
        .scroll_offset(10_000.0)
        .overscan(8)
        .window();

      assert_eq!(window.end_row, 100);
      assert!(window.first_row <= window.end_row);
      assert_eq!(window.trailing, 0.0);
    }

    #[test]
    fn it_falls_back_to_one_overscan_screenful_without_a_viewport() {
      let window = VirtualListConfig::new(1_000, 40.0).overscan(8).window();

      assert_eq!(window.first_row, 0);
      assert_eq!(window.end_row, 16);
    }

    #[test]
    fn it_falls_back_to_overscan_for_a_huge_finite_viewport() {
      let window = VirtualListConfig::new(1_000, 40.0)
        .viewport_height(1e30)
        .overscan(8)
        .window();

      assert_eq!(window.first_row, 0);
      assert_eq!(window.end_row, 16);
    }

    #[test]
    fn it_falls_back_to_overscan_for_a_nan_viewport() {
      let window = VirtualListConfig::new(1_000, 40.0)
        .viewport_height(f32::NAN)
        .overscan(8)
        .window();

      assert_eq!(window.first_row, 0);
      assert_eq!(window.end_row, 16);
    }

    #[test]
    fn it_falls_back_to_overscan_for_a_negative_viewport() {
      let window = VirtualListConfig::new(1_000, 40.0)
        .viewport_height(-400.0)
        .overscan(8)
        .window();

      assert_eq!(window.first_row, 0);
      assert_eq!(window.end_row, 16);
    }

    #[test]
    fn it_falls_back_to_overscan_for_a_zero_viewport() {
      let window = VirtualListConfig::new(1_000, 40.0)
        .viewport_height(0.0)
        .overscan(8)
        .window();

      assert_eq!(window.first_row, 0);
      assert_eq!(window.end_row, 16);
    }

    #[test]
    fn it_falls_back_to_overscan_for_an_infinite_viewport() {
      let window = VirtualListConfig::new(1_000, 40.0)
        .viewport_height(f32::INFINITY)
        .overscan(8)
        .window();

      assert_eq!(window.first_row, 0);
      assert_eq!(window.end_row, 16);
    }

    #[test]
    fn it_groups_items_into_rows_for_a_grid() {
      let window = VirtualListConfig::new(100, 120.0)
        .items_per_row(4)
        .viewport_height(360.0)
        .scroll_offset(0.0)
        .overscan(2)
        .window();

      assert_eq!(window.first_row, 0);
      assert_eq!(window.end_row, 6);
      assert_eq!(window.trailing, 19.0 * 120.0);
    }

    #[test]
    fn it_keeps_the_leading_and_trailing_spacers_summing_to_the_off_window_height() {
      let total = 500usize;
      let row_h = 30.0f32;
      let config = VirtualListConfig::new(total, row_h)
        .viewport_height(300.0)
        .scroll_offset(3_000.0);
      let window = config.window();

      let materialized = window.len() as f32 * row_h;
      let full_height = total as f32 * row_h;

      assert_eq!(window.leading + materialized + window.trailing, full_height);
    }

    #[test]
    fn it_returns_an_empty_window_for_no_items() {
      let window = VirtualListConfig::new(0, 40.0).viewport_height(400.0).window();

      assert_eq!(window.first_row, 0);
      assert_eq!(window.end_row, 0);
      assert_eq!(window.leading, 0.0);
      assert_eq!(window.trailing, 0.0);
      assert!(window.is_empty());
    }

    #[test]
    fn it_treats_a_zero_estimated_height_as_one_pixel() {
      let window = VirtualListConfig::new(10, 0.0).viewport_height(100.0).window();

      assert!(window.end_row <= 10);
    }

    #[test]
    fn it_windows_from_the_top_with_overscan_and_a_full_trailing_spacer() {
      let window = VirtualListConfig::new(1_000, 40.0)
        .viewport_height(400.0)
        .scroll_offset(0.0)
        .overscan(8)
        .window();

      assert_eq!(window.first_row, 0);
      assert_eq!(window.end_row, 19);
      assert_eq!(window.leading, 0.0);
      assert_eq!(window.trailing, 981.0 * 40.0);
    }
  }
}
