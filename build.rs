use std::process::Command;

fn main() {
  let git_sha = Command::new("git")
    .args(["rev-parse", "--short", "HEAD"])
    .output()
    .ok()
    .filter(|output| output.status.success())
    .and_then(|output| String::from_utf8(output.stdout).ok())
    .map(|sha| sha.trim().to_owned())
    .filter(|sha| !sha.is_empty())
    .unwrap_or_else(|| "unknown".to_owned());
  println!("cargo:rustc-env=POD_GIT_SHA={git_sha}");

  let build_date = build_date();
  println!("cargo:rustc-env=POD_BUILD_DATE={build_date}");

  println!("cargo:rerun-if-changed=.git/HEAD");
  println!("cargo:rerun-if-changed=build.rs");
}

fn build_date() -> String {
  let secs = std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .map(|d| d.as_secs())
    .unwrap_or(0);
  let days = (secs / 86_400) as i64;

  let z = days + 719_468;
  let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
  let doe = z - era * 146_097;
  let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
  let year = yoe + era * 400;
  let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
  let mp = (5 * doy + 2) / 153;
  let day = doy - (153 * mp + 2) / 5 + 1;
  let month = if mp < 10 { mp + 3 } else { mp - 9 };
  let year = if month <= 2 { year + 1 } else { year };

  format!("{year:04}-{month:02}-{day:02}")
}
