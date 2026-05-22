//! Domain model for character kill and loss log entries.

use getset::Getters;

/// A kill or loss record from the character's combat history.
#[derive(Clone, Debug, Getters, PartialEq)]
pub struct KillEntry {
  /// Number of attackers on the killmail.
  #[get = "pub"]
  attacker_count: i32,
  /// Whether the character landed the final blow.
  #[get = "pub"]
  final_blow: bool,
  /// Whether this is a kill (`true`) or a loss (`false`).
  #[get = "pub"]
  is_kill: bool,
  /// ISO-8601 timestamp of the kill event.
  #[get = "pub"]
  kill_time: String,
  /// EVE killmail identifier.
  #[get = "pub"]
  killmail_id: i32,
  /// Resolved name of the ship type flown.
  #[get = "pub"]
  ship_name: String,
  /// Resolved name of the solar system where the kill occurred.
  #[get = "pub"]
  system_name: String,
  /// Security status of the solar system (e.g., 0.5 for high-sec boundary).
  #[get = "pub"]
  system_sec: f32,
  /// Estimated ISK value of the killmail.
  #[get = "pub"]
  value_isk: f64,
  /// Name of the victim's corporation.
  #[get = "pub"]
  victim_corp_name: String,
  /// Resolved name of the victim.
  #[get = "pub"]
  victim_name: String,
}

impl KillEntry {
  /// Creates a new kill entry.
  #[allow(clippy::too_many_arguments)]
  pub fn new(
    killmail_id: i32,
    is_kill: bool,
    ship_name: impl Into<String>,
    victim_name: impl Into<String>,
    victim_corp_name: impl Into<String>,
    system_name: impl Into<String>,
    system_sec: f32,
    value_isk: f64,
    attacker_count: i32,
    final_blow: bool,
    kill_time: impl Into<String>,
  ) -> Self {
    Self {
      attacker_count,
      final_blow,
      is_kill,
      kill_time: kill_time.into(),
      killmail_id,
      ship_name: ship_name.into(),
      system_name: system_name.into(),
      system_sec,
      value_isk,
      victim_corp_name: victim_corp_name.into(),
      victim_name: victim_name.into(),
    }
  }

  /// Sets the attacker count.
  pub fn set_attacker_count(&mut self, attacker_count: i32) -> &mut Self {
    self.attacker_count = attacker_count;
    self
  }

  /// Sets whether the character landed the final blow.
  pub fn set_final_blow(&mut self, final_blow: bool) -> &mut Self {
    self.final_blow = final_blow;
    self
  }

  /// Sets whether this is a kill or a loss.
  pub fn set_is_kill(&mut self, is_kill: bool) -> &mut Self {
    self.is_kill = is_kill;
    self
  }

  /// Sets the kill timestamp.
  pub fn set_kill_time(&mut self, kill_time: impl Into<String>) -> &mut Self {
    self.kill_time = kill_time.into();
    self
  }

  /// Sets the killmail ID.
  pub fn set_killmail_id(&mut self, killmail_id: i32) -> &mut Self {
    self.killmail_id = killmail_id;
    self
  }

  /// Sets the ship name.
  pub fn set_ship_name(&mut self, ship_name: impl Into<String>) -> &mut Self {
    self.ship_name = ship_name.into();
    self
  }

  /// Sets the solar system name.
  pub fn set_system_name(&mut self, system_name: impl Into<String>) -> &mut Self {
    self.system_name = system_name.into();
    self
  }

  /// Sets the solar system security status.
  pub fn set_system_sec(&mut self, system_sec: f32) -> &mut Self {
    self.system_sec = system_sec;
    self
  }

  /// Sets the estimated ISK value.
  pub fn set_value_isk(&mut self, value_isk: f64) -> &mut Self {
    self.value_isk = value_isk;
    self
  }

  /// Sets the victim's corporation name.
  pub fn set_victim_corp_name(&mut self, victim_corp_name: impl Into<String>) -> &mut Self {
    self.victim_corp_name = victim_corp_name.into();
    self
  }

  /// Sets the victim's name.
  pub fn set_victim_name(&mut self, victim_name: impl Into<String>) -> &mut Self {
    self.victim_name = victim_name.into();
    self
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  fn make_kill() -> KillEntry {
    KillEntry::new(
      123_456_789,
      true,
      "Rifter",
      "Victim McVictimface",
      "Test Corp",
      "Jita",
      0.9,
      150_000_000.0,
      3,
      true,
      "2024-01-15T12:30:00Z",
    )
  }

  mod set_is_kill {
    use super::*;

    #[test]
    fn it_sets_kill_flag_to_false_for_loss() {
      let mut k = make_kill();
      k.set_is_kill(false);
      assert!(!k.is_kill());
    }
  }

  mod set_value_isk {
    use super::*;

    #[test]
    fn it_sets_the_isk_value() {
      let mut k = make_kill();
      k.set_value_isk(200_000_000.0);
      assert_eq!(*k.value_isk(), 200_000_000.0_f64);
    }
  }
}
