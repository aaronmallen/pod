use serde::{Deserialize, Serialize};

use crate::{services::pod_pack, store::model::FacilityIntel};

pub const FILE_NAME: &str = "facility-intel.pfi";
pub const PACK_EXTENSION: &str = "pfi";
pub const PACK_VERSION: u32 = 1;

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct PackEnvelope {
  #[serde(default)]
  pub facilities: Vec<PortableFacility>,
  #[serde(default)]
  pub format: String,
  #[serde(default)]
  pub version: u32,
}

#[derive(Debug, Eq, PartialEq, thiserror::Error)]
pub enum ParseError {
  #[error("pack contains no facilities")]
  Empty,
  #[error("not a pod facility intel pack")]
  NotAPack,
  #[error("unsupported pack version")]
  UnsupportedVersion,
  #[error("wrong pack format")]
  WrongFormat,
}

#[derive(Clone, Debug, Default, Deserialize, PartialEq, Serialize)]
pub struct PortableFacility {
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub eft: Option<String>,
  pub facility_id: i64,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub name: Option<String>,
  #[serde(default)]
  pub rigs: [Option<i64>; 3],
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub solar_system_id: Option<i64>,
  #[serde(default, skip_serializing_if = "Option::is_none")]
  pub type_id: Option<i64>,
}

impl PortableFacility {
  pub fn to_intel(&self) -> FacilityIntel {
    FacilityIntel {
      eft: self.eft.clone(),
      facility_id: self.facility_id,
      name: self.name.clone(),
      rig_1_type_id: self.rigs[0],
      rig_2_type_id: self.rigs[1],
      rig_3_type_id: self.rigs[2],
      solar_system_id: self.solar_system_id,
      type_id: self.type_id,
    }
  }
}

pub fn build_pack(facilities: Vec<PortableFacility>) -> PackEnvelope {
  PackEnvelope {
    facilities,
    format: pod_pack::TAG_FACILITY_INTEL.to_owned(),
    version: PACK_VERSION,
  }
}

pub fn encode_pack(pack: &PackEnvelope) -> Result<String, pod_pack::EncodeError> {
  pod_pack::encode(pod_pack::TAG_FACILITY_INTEL, PACK_VERSION, pack)
}

pub fn parse_pack(input: &str) -> Result<PackEnvelope, ParseError> {
  let pack: PackEnvelope = pod_pack::decode(pod_pack::TAG_FACILITY_INTEL, PACK_VERSION, input)?;
  // pod_pack::decode already checked the outer envelope's framing tag; this checks the payload's
  // own `format` field, which that framing doesn't constrain and can disagree with it.
  if pack.format != pod_pack::TAG_FACILITY_INTEL {
    return Err(ParseError::WrongFormat);
  }
  if pack.facilities.is_empty() {
    return Err(ParseError::Empty);
  }
  Ok(pack)
}

pub fn portable_facility(intel: &FacilityIntel) -> PortableFacility {
  PortableFacility {
    eft: intel.eft.clone(),
    facility_id: intel.facility_id,
    name: intel.name.clone(),
    rigs: [intel.rig_1_type_id, intel.rig_2_type_id, intel.rig_3_type_id],
    solar_system_id: intel.solar_system_id,
    type_id: intel.type_id,
  }
}

impl From<pod_pack::DecodeError> for ParseError {
  fn from(error: pod_pack::DecodeError) -> Self {
    match error {
      pod_pack::DecodeError::UnsupportedVersion {
        ..
      } => ParseError::UnsupportedVersion,
      pod_pack::DecodeError::WrongFormat {
        ..
      } => ParseError::WrongFormat,
      _ => ParseError::NotAPack,
    }
  }
}

#[cfg(test)]
mod tests {
  use std::io::Write;

  use base64::{Engine as _, engine::general_purpose::STANDARD};
  use flate2::{Compression, write::DeflateEncoder};

  use super::*;

  const MAGIC: [u8; 8] = *b"PODPACK\0";

  fn build_frame(tag: &str, version: u32, checksum: u32, json: &[u8]) -> Vec<u8> {
    let mut frame = Vec::new();
    frame.extend_from_slice(&MAGIC);
    frame.push(tag.len() as u8);
    frame.extend_from_slice(tag.as_bytes());
    frame.extend_from_slice(&version.to_le_bytes());
    frame.extend_from_slice(&checksum.to_le_bytes());
    frame.extend_from_slice(json);
    frame
  }

  fn pack_raw(frame: &[u8]) -> String {
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
    encoder.write_all(frame).unwrap();
    STANDARD.encode(encoder.finish().unwrap())
  }

  fn sample_intel() -> FacilityIntel {
    FacilityIntel {
      eft: Some("[Astrahus, Home]\nStandup Cloning Center I".to_owned()),
      facility_id: 1_035_466_617_946,
      name: Some("Jita Trade Citadel".to_owned()),
      rig_1_type_id: Some(37_180),
      rig_2_type_id: None,
      rig_3_type_id: Some(43_704),
      solar_system_id: Some(30_000_142),
      type_id: Some(35_834),
    }
  }

  fn sample_pack() -> PackEnvelope {
    build_pack(vec![
      portable_facility(&sample_intel()),
      portable_facility(&FacilityIntel {
        eft: None,
        facility_id: 60_003_760,
        name: None,
        rig_1_type_id: None,
        rig_2_type_id: None,
        rig_3_type_id: None,
        solar_system_id: None,
        type_id: None,
      }),
    ])
  }

  mod round_trip {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_round_trips_a_pack_through_the_codec() {
      let pack = sample_pack();
      let encoded = encode_pack(&pack).unwrap();

      let decoded = parse_pack(&encoded).unwrap();

      assert_eq!(decoded, pack);
    }

    #[test]
    fn it_preserves_empty_rig_slots_and_missing_snapshots() {
      let pack = sample_pack();

      let decoded = parse_pack(&encode_pack(&pack).unwrap()).unwrap();

      assert_eq!(decoded.facilities[0].rigs, [Some(37_180), None, Some(43_704)]);
      assert_eq!(decoded.facilities[0].name.as_deref(), Some("Jita Trade Citadel"));
      assert_eq!(decoded.facilities[0].solar_system_id, Some(30_000_142));
      assert_eq!(decoded.facilities[0].type_id, Some(35_834));

      assert_eq!(decoded.facilities[1].rigs, [None, None, None]);
      assert_eq!(decoded.facilities[1].name, None);
      assert_eq!(decoded.facilities[1].solar_system_id, None);
      assert_eq!(decoded.facilities[1].type_id, None);
    }

    #[test]
    fn it_round_trips_a_non_null_eft() {
      let pack = sample_pack();

      let decoded = parse_pack(&encode_pack(&pack).unwrap()).unwrap();

      assert_eq!(
        decoded.facilities[0].eft.as_deref(),
        Some("[Astrahus, Home]\nStandup Cloning Center I")
      );
      assert_eq!(decoded.facilities[1].eft, None);
    }

    #[test]
    fn it_decodes_a_pack_without_an_eft_field_as_none() {
      let json = serde_json::to_vec(&build_pack(vec![portable_facility(&FacilityIntel {
        eft: None,
        facility_id: 60_003_760,
        name: None,
        rig_1_type_id: None,
        rig_2_type_id: None,
        rig_3_type_id: None,
        solar_system_id: None,
        type_id: None,
      })]))
      .unwrap();
      assert!(!String::from_utf8_lossy(&json).contains("eft"));
      let frame = build_frame(
        pod_pack::TAG_FACILITY_INTEL,
        PACK_VERSION,
        crc32fast::hash(&json),
        &json,
      );

      let decoded = parse_pack(&pack_raw(&frame)).unwrap();

      assert_eq!(decoded.facilities[0].eft, None);
    }

    #[test]
    fn it_round_trips_intel_rows_through_the_portable_shape() {
      let intel = sample_intel();

      let decoded = parse_pack(&encode_pack(&build_pack(vec![portable_facility(&intel)])).unwrap()).unwrap();

      assert_eq!(decoded.facilities[0].to_intel(), intel);
    }
  }

  mod parse_pack {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_rejects_plain_text() {
      assert_eq!(
        parse_pack("just some plain text, not a pack"),
        Err(ParseError::NotAPack)
      );
    }

    #[test]
    fn it_rejects_bare_json() {
      let json = serde_json::to_string(&sample_pack()).unwrap();

      assert_eq!(parse_pack(&json), Err(ParseError::NotAPack));
    }

    #[test]
    fn it_rejects_a_truncated_pack() {
      let encoded = encode_pack(&sample_pack()).unwrap();

      let result = parse_pack(&encoded[..encoded.len() / 2]);

      assert_eq!(result, Err(ParseError::NotAPack));
    }

    #[test]
    fn it_rejects_a_frame_truncated_inside_the_header() {
      let json = serde_json::to_vec(&sample_pack()).unwrap();
      let frame = build_frame(
        pod_pack::TAG_FACILITY_INTEL,
        PACK_VERSION,
        crc32fast::hash(&json),
        &json,
      );
      let truncated = &frame[..MAGIC.len() + 4];

      assert_eq!(parse_pack(&pack_raw(truncated)), Err(ParseError::NotAPack));
    }

    #[test]
    fn it_rejects_a_wrong_magic_header() {
      let json = serde_json::to_vec(&sample_pack()).unwrap();
      let mut frame = build_frame(
        pod_pack::TAG_FACILITY_INTEL,
        PACK_VERSION,
        crc32fast::hash(&json),
        &json,
      );
      frame[..MAGIC.len()].copy_from_slice(b"NOTAPACK");

      assert_eq!(parse_pack(&pack_raw(&frame)), Err(ParseError::NotAPack));
    }

    #[test]
    fn it_rejects_a_bad_checksum() {
      let json = serde_json::to_vec(&sample_pack()).unwrap();
      let mut tampered = json.clone();
      let last = tampered.len() - 5;
      tampered[last] ^= 0xff;
      let frame = build_frame(
        pod_pack::TAG_FACILITY_INTEL,
        PACK_VERSION,
        crc32fast::hash(&json),
        &tampered,
      );

      assert_eq!(parse_pack(&pack_raw(&frame)), Err(ParseError::NotAPack));
    }

    #[test]
    fn it_rejects_a_wrong_format_tag() {
      let encoded = pod_pack::encode(pod_pack::TAG_SKILL_PLAN, PACK_VERSION, &sample_pack()).unwrap();

      assert_eq!(parse_pack(&encoded), Err(ParseError::WrongFormat));
    }

    #[test]
    fn it_rejects_a_mismatched_envelope_format_field() {
      let mut pack = sample_pack();
      pack.format = "pod.skill-plan".to_owned();
      let encoded = encode_pack(&pack).unwrap();

      assert_eq!(parse_pack(&encoded), Err(ParseError::WrongFormat));
    }

    #[test]
    fn it_rejects_an_unsupported_version() {
      let encoded = pod_pack::encode(pod_pack::TAG_FACILITY_INTEL, PACK_VERSION + 1, &sample_pack()).unwrap();

      assert_eq!(parse_pack(&encoded), Err(ParseError::UnsupportedVersion));
    }

    #[test]
    fn it_rejects_an_empty_pack() {
      let encoded = encode_pack(&build_pack(Vec::new())).unwrap();

      assert_eq!(parse_pack(&encoded), Err(ParseError::Empty));
    }

    #[test]
    fn it_rejects_empty_input() {
      assert_eq!(parse_pack(""), Err(ParseError::NotAPack));
    }
  }

  mod file_name {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_carries_the_pack_extension() {
      assert_eq!(FILE_NAME, format!("facility-intel.{PACK_EXTENSION}"));
    }
  }

  mod encode_pack {
    use super::*;

    #[test]
    fn it_is_not_plain_text_readable() {
      let encoded = encode_pack(&sample_pack()).unwrap();

      assert!(!encoded.contains("Jita Trade Citadel"));
      assert!(!encoded.contains("facility_id"));
      assert!(!encoded.contains(pod_pack::TAG_FACILITY_INTEL));
      assert!(STANDARD.decode(&encoded).is_ok());
    }
  }
}
