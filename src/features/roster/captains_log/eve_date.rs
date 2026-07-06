use chrono::{Datelike, NaiveDate};

const YC_EPOCH_OFFSET: i32 = 1898;

pub(super) fn label(date: NaiveDate) -> String {
  format!("YC {}", yc_year(date.year()))
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
  }

  mod label {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_formats_a_yc_year_label() {
      let date = NaiveDate::from_ymd_opt(2026, 7, 6).unwrap();

      assert_eq!(label(date), "YC 128");
    }
  }
}
