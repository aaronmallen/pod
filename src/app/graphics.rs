use wgpu::{AdapterInfo, Backends, Instance, InstanceDescriptor};

struct AdapterSummary {
  backend: String,
  device_type: String,
  name: String,
}

impl AdapterSummary {
  fn software_only(summaries: &[Self]) -> bool {
    !summaries.is_empty() && summaries.iter().all(|summary| summary.device_type == "Cpu")
  }
}

impl From<&AdapterInfo> for AdapterSummary {
  fn from(info: &AdapterInfo) -> Self {
    Self {
      backend: format!("{:?}", info.backend),
      device_type: format!("{:?}", info.device_type),
      name: info.name.clone(),
    }
  }
}

pub fn probe() {
  let instance = Instance::new(&InstanceDescriptor::default());
  let adapters = instance.enumerate_adapters(Backends::all());

  if adapters.is_empty() {
    tracing::warn!(
      target: "pod::graphics",
      "no wgpu graphics adapters found; iced will fall back to the tiny_skia software renderer",
    );
    return;
  }

  let summaries: Vec<AdapterSummary> = adapters
    .iter()
    .map(|adapter| AdapterSummary::from(&adapter.get_info()))
    .collect();

  for summary in &summaries {
    tracing::info!(
      target: "pod::graphics",
      backend = %summary.backend,
      device_type = %summary.device_type,
      name = %summary.name,
      "wgpu graphics adapter available",
    );
  }

  tracing::info!(
    target: "pod::graphics",
    adapter_count = summaries.len(),
    software_only = AdapterSummary::software_only(&summaries),
    "enumerated wgpu graphics adapters",
  );
}

#[cfg(test)]
mod tests {
  use super::*;

  mod software_only {
    use super::*;

    fn summary(device_type: &str) -> AdapterSummary {
      AdapterSummary {
        backend: "Gl".to_string(),
        device_type: device_type.to_string(),
        name: "test adapter".to_string(),
      }
    }

    #[test]
    fn it_returns_false_when_any_adapter_is_a_gpu() {
      let summaries = [summary("Cpu"), summary("DiscreteGpu")];

      assert!(!AdapterSummary::software_only(&summaries));
    }

    #[test]
    fn it_returns_false_when_there_are_no_adapters() {
      let summaries: [AdapterSummary; 0] = [];

      assert!(!AdapterSummary::software_only(&summaries));
    }

    #[test]
    fn it_returns_true_when_every_adapter_is_a_cpu() {
      let summaries = [summary("Cpu"), summary("Cpu")];

      assert!(AdapterSummary::software_only(&summaries));
    }
  }
}
