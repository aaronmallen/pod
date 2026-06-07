use super::single_instance;

pub fn install() {
  if let Some(lock) = single_instance::try_become_primary() {
    single_instance::spawn_listener(lock, super::deliver);
  }
}
