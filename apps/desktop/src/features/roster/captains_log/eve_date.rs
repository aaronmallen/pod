use chrono::{Datelike, NaiveDate};

const YC_EPOCH_OFFSET: i32 = 1898;

pub(super) fn label(date: NaiveDate) -> String {
  format!("YC{}.{:02}.{:02}", yc_year(date.year()), date.month(), date.day())
}

pub(super) fn yc_year(gregorian_year: i32) -> i32 {
  gregorian_year - YC_EPOCH_OFFSET
}

#[cfg(test)]
mod tests {
  use super::*;

  mod yc_year {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_offsets_the_gregorian_year_by_the_yc_epoch() {
      assert_eq!(yc_year(2026), 128);
      assert_eq!(yc_year(1898), 0);
    }

    #[test]
    fn it_rolls_the_yc_year_across_a_gregorian_boundary() {
      assert_eq!(yc_year(1898), 0);
      assert_eq!(yc_year(1899), 1);
    }
  }

  mod label {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_formats_the_eve_date_as_yc_year_month_day() {
      let date = NaiveDate::from_ymd_opt(2026, 7, 5).unwrap();

      assert_eq!(label(date), "YC128.07.05");
    }

    #[test]
    fn it_zero_pads_single_digit_months_and_days() {
      let date = NaiveDate::from_ymd_opt(2026, 1, 6).unwrap();

      assert_eq!(label(date), "YC128.01.06");
    }

    #[test]
    fn it_labels_the_yc_epoch_boundary() {
      let last = NaiveDate::from_ymd_opt(1898, 12, 31).unwrap();
      let first = NaiveDate::from_ymd_opt(1899, 1, 1).unwrap();

      assert_eq!(label(last), "YC0.12.31");
      assert_eq!(label(first), "YC1.01.01");
    }
  }
}
