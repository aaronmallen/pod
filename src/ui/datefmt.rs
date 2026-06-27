use chrono::Weekday;

const MONTHS_LONG: [&str; 12] = [
  "date.month.long_jan",
  "date.month.long_feb",
  "date.month.long_mar",
  "date.month.long_apr",
  "date.month.long_may",
  "date.month.long_jun",
  "date.month.long_jul",
  "date.month.long_aug",
  "date.month.long_sep",
  "date.month.long_oct",
  "date.month.long_nov",
  "date.month.long_dec",
];

const MONTHS_SHORT: [&str; 12] = [
  "date.month.short_jan",
  "date.month.short_feb",
  "date.month.short_mar",
  "date.month.short_apr",
  "date.month.short_may",
  "date.month.short_jun",
  "date.month.short_jul",
  "date.month.short_aug",
  "date.month.short_sep",
  "date.month.short_oct",
  "date.month.short_nov",
  "date.month.short_dec",
];

const WEEKDAYS_LONG: [&str; 7] = [
  "date.weekday.long_mon",
  "date.weekday.long_tue",
  "date.weekday.long_wed",
  "date.weekday.long_thu",
  "date.weekday.long_fri",
  "date.weekday.long_sat",
  "date.weekday.long_sun",
];

const WEEKDAYS_SHORT: [&str; 7] = [
  "date.weekday.short_mon",
  "date.weekday.short_tue",
  "date.weekday.short_wed",
  "date.weekday.short_thu",
  "date.weekday.short_fri",
  "date.weekday.short_sat",
  "date.weekday.short_sun",
];

pub fn month_long(month: u32) -> String {
  t!(MONTHS_LONG[month_index(month)]).into_owned()
}

pub fn month_short(month: u32) -> String {
  t!(MONTHS_SHORT[month_index(month)]).into_owned()
}

pub fn weekday_long(weekday: Weekday) -> String {
  t!(WEEKDAYS_LONG[weekday.num_days_from_monday() as usize]).into_owned()
}

pub fn weekday_short(weekday: Weekday) -> String {
  t!(WEEKDAYS_SHORT[weekday.num_days_from_monday() as usize]).into_owned()
}

fn month_index(month: u32) -> usize {
  (month.clamp(1, 12) - 1) as usize
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::i18n::{Language, set_locale};

  mod month_long {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_returns_the_localized_long_month_name() {
      set_locale(Language::En);

      assert_eq!(month_long(1), "January");
      assert_eq!(month_long(6), "June");
      assert_eq!(month_long(12), "December");
    }

    #[test]
    fn it_clamps_a_month_below_the_range() {
      set_locale(Language::En);

      assert_eq!(month_long(0), "January");
    }

    #[test]
    fn it_clamps_a_month_above_the_range() {
      set_locale(Language::En);

      assert_eq!(month_long(13), "December");
    }
  }

  mod month_short {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_returns_the_localized_short_month_name() {
      set_locale(Language::En);

      assert_eq!(month_short(1), "Jan");
      assert_eq!(month_short(9), "Sep");
      assert_eq!(month_short(12), "Dec");
    }

    #[test]
    fn it_clamps_an_out_of_range_month() {
      set_locale(Language::En);

      assert_eq!(month_short(0), "Jan");
      assert_eq!(month_short(13), "Dec");
    }
  }

  mod weekday_long {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_returns_the_localized_long_weekday_name() {
      set_locale(Language::En);

      assert_eq!(weekday_long(Weekday::Mon), "Monday");
      assert_eq!(weekday_long(Weekday::Sun), "Sunday");
    }
  }

  mod weekday_short {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_returns_the_localized_short_weekday_name() {
      set_locale(Language::En);

      assert_eq!(weekday_short(Weekday::Mon), "Mon");
      assert_eq!(weekday_short(Weekday::Sat), "Sat");
    }
  }
}
