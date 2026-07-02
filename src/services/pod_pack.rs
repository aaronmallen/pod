//! Portable, tamper-evident codec shared by every "share this thing" pack format (`.pbr` today; `.pfi` and
//! `.psp` later). See ADR-0045.
//!
//! Wire format: `PODPACK\0` magic + 1-byte tag length + tag bytes + u32 LE version + u32 LE CRC32 (over the JSON
//! only, computed before the frame is assembled) + JSON payload, then raw-deflated and base64-encoded. `decode`
//! validates every layer, never panics, and never returns a partial payload on bad input; `version` must match
//! exactly, not merely be supported or lower.

use std::io::{Read, Write};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use flate2::{Compression, read::DeflateDecoder, write::DeflateEncoder};
use serde::{Serialize, de::DeserializeOwned};

pub const TAG_BUDGET_RULES: &str = "pod.budget-rules";
#[cfg_attr(not(test), expect(dead_code))]
pub const TAG_FACILITY_INTEL: &str = "pod.facility-intel";
#[cfg_attr(not(test), expect(dead_code))]
pub const TAG_SKILL_PLAN: &str = "pod.skill-plan";

const MAGIC: [u8; 8] = *b"PODPACK\0";

#[derive(Debug, thiserror::Error)]
pub enum DecodeError {
  #[error("not a pod pack: invalid base64: {0}")]
  Base64(#[from] base64::DecodeError),
  #[error("pod pack checksum mismatch (expected {expected:08x}, found {found:08x})")]
  ChecksumMismatch { expected: u32, found: u32 },
  #[error("not a pod pack: invalid compressed data: {0}")]
  Inflate(#[source] std::io::Error),
  #[error("pod pack envelope is not valid JSON: {0}")]
  Json(#[from] serde_json::Error),
  #[error("not a pod pack: magic header missing or wrong")]
  NotAPack,
  #[error("pod pack is truncated")]
  Truncated,
  #[error("unsupported pod pack version (found {found}, supported {supported})")]
  UnsupportedVersion { found: u32, supported: u32 },
  #[error("wrong pod pack format (expected {expected:?}, found {found:?})")]
  WrongFormat { expected: String, found: String },
}

#[derive(Debug, thiserror::Error)]
pub enum EncodeError {
  #[error("failed to compress pod pack: {0}")]
  Compress(#[source] std::io::Error),
  #[error("failed to serialize pod pack envelope: {0}")]
  Json(#[from] serde_json::Error),
  #[error("pod pack format tag is too long ({0} bytes, max 255)")]
  TagTooLong(usize),
}

pub fn decode<T: DeserializeOwned>(tag: &str, version: u32, input: &str) -> Result<T, DecodeError> {
  let compressed = STANDARD.decode(input.trim())?;
  let mut frame = Vec::new();
  DeflateDecoder::new(compressed.as_slice())
    .read_to_end(&mut frame)
    .map_err(DecodeError::Inflate)?;

  if frame.len() < MAGIC.len() || frame[..MAGIC.len()] != MAGIC {
    return Err(DecodeError::NotAPack);
  }
  let rest = &frame[MAGIC.len()..];
  let (&tag_len, rest) = rest.split_first().ok_or(DecodeError::Truncated)?;
  let tag_len = usize::from(tag_len);
  if rest.len() < tag_len + 8 {
    // 8 = 4-byte version + 4-byte checksum that must follow the tag.
    return Err(DecodeError::Truncated);
  }
  let (tag_bytes, rest) = rest.split_at(tag_len);
  let (version_bytes, rest) = rest.split_at(4);
  let (checksum_bytes, json) = rest.split_at(4);

  let found_tag = String::from_utf8_lossy(tag_bytes);
  if found_tag != tag {
    return Err(DecodeError::WrongFormat {
      expected: tag.to_string(),
      found: found_tag.into_owned(),
    });
  }

  let found_version = u32::from_le_bytes(version_bytes.try_into().map_err(|_| DecodeError::Truncated)?);
  if found_version != version {
    return Err(DecodeError::UnsupportedVersion {
      found: found_version,
      supported: version,
    });
  }

  let expected_checksum = u32::from_le_bytes(checksum_bytes.try_into().map_err(|_| DecodeError::Truncated)?);
  let found_checksum = crc32fast::hash(json);
  if found_checksum != expected_checksum {
    return Err(DecodeError::ChecksumMismatch {
      expected: expected_checksum,
      found: found_checksum,
    });
  }

  Ok(serde_json::from_slice(json)?)
}

pub fn encode<T: Serialize>(tag: &str, version: u32, envelope: &T) -> Result<String, EncodeError> {
  let Ok(tag_len) = u8::try_from(tag.len()) else {
    return Err(EncodeError::TagTooLong(tag.len()));
  };

  let json = serde_json::to_vec(envelope)?;
  let checksum = crc32fast::hash(&json);

  let mut frame = Vec::with_capacity(MAGIC.len() + 1 + tag.len() + 8 + json.len());
  frame.extend_from_slice(&MAGIC);
  frame.push(tag_len);
  frame.extend_from_slice(tag.as_bytes());
  frame.extend_from_slice(&version.to_le_bytes());
  frame.extend_from_slice(&checksum.to_le_bytes());
  frame.extend_from_slice(&json);

  let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
  encoder.write_all(&frame).map_err(EncodeError::Compress)?;
  let compressed = encoder.finish().map_err(EncodeError::Compress)?;

  Ok(STANDARD.encode(compressed))
}

#[cfg(test)]
mod tests {
  use serde::Deserialize;

  use super::*;

  #[derive(Debug, Deserialize, PartialEq, Serialize)]
  struct TestEnvelope {
    format: String,
    name: String,
    rules: Vec<String>,
    version: u32,
  }

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

  fn sample_envelope() -> TestEnvelope {
    TestEnvelope {
      format: TAG_BUDGET_RULES.to_string(),
      name: "Corp starter rules".to_string(),
      rules: vec!["fuel".to_string(), "ammo".to_string()],
      version: 1,
    }
  }

  mod decode {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_round_trips_an_envelope() {
      let envelope = sample_envelope();
      let encoded = encode(TAG_BUDGET_RULES, 1, &envelope).unwrap();

      let decoded: TestEnvelope = decode(TAG_BUDGET_RULES, 1, &encoded).unwrap();

      assert_eq!(decoded, envelope);
    }

    #[test]
    fn it_round_trips_every_known_tag() {
      let envelope = sample_envelope();

      for tag in [TAG_BUDGET_RULES, TAG_FACILITY_INTEL, TAG_SKILL_PLAN] {
        let encoded = encode(tag, 3, &envelope).unwrap();
        let decoded: TestEnvelope = decode(tag, 3, &encoded).unwrap();

        assert_eq!(decoded, envelope);
      }
    }

    #[test]
    fn it_rejects_invalid_base64() {
      let result: Result<TestEnvelope, _> = decode(TAG_BUDGET_RULES, 1, "!!!not base64!!!");

      assert!(matches!(result, Err(DecodeError::Base64(_))));
    }

    #[test]
    fn it_rejects_plain_text() {
      let result: Result<TestEnvelope, _> = decode(TAG_BUDGET_RULES, 1, "just some plain text, not a pack");

      assert!(result.is_err());
    }

    #[test]
    fn it_rejects_bare_json() {
      let json = serde_json::to_string(&sample_envelope()).unwrap();

      let result: Result<TestEnvelope, _> = decode(TAG_BUDGET_RULES, 1, &json);

      assert!(result.is_err());
    }

    #[test]
    fn it_rejects_base64_that_does_not_inflate() {
      let input = STANDARD.encode(b"random uncompressed garbage bytes");

      let result: Result<TestEnvelope, _> = decode(TAG_BUDGET_RULES, 1, &input);

      assert!(matches!(result, Err(DecodeError::Inflate(_))));
    }

    #[test]
    fn it_rejects_a_wrong_magic_header() {
      let mut frame = build_frame(TAG_BUDGET_RULES, 1, 0, b"{}");
      frame[..8].copy_from_slice(b"NOTAPACK");

      let result: Result<TestEnvelope, _> = decode(TAG_BUDGET_RULES, 1, &pack_raw(&frame));

      assert!(matches!(result, Err(DecodeError::NotAPack)));
    }

    #[test]
    fn it_rejects_a_wrong_format_tag() {
      let encoded = encode(TAG_SKILL_PLAN, 1, &sample_envelope()).unwrap();

      let result: Result<TestEnvelope, _> = decode(TAG_BUDGET_RULES, 1, &encoded);

      assert!(matches!(
        result,
        Err(DecodeError::WrongFormat { expected, found })
          if expected == TAG_BUDGET_RULES && found == TAG_SKILL_PLAN
      ));
    }

    #[test]
    fn it_rejects_an_unsupported_future_version() {
      let encoded = encode(TAG_BUDGET_RULES, 2, &sample_envelope()).unwrap();

      let result: Result<TestEnvelope, _> = decode(TAG_BUDGET_RULES, 1, &encoded);

      assert!(matches!(
        result,
        Err(DecodeError::UnsupportedVersion {
          found: 2,
          supported: 1
        })
      ));
    }

    #[test]
    fn it_rejects_a_checksum_mismatch() {
      let json = serde_json::to_vec(&sample_envelope()).unwrap();
      let mut tampered = json.clone();
      let last = tampered.len() - 10;
      tampered[last] ^= 0xff;
      let frame = build_frame(TAG_BUDGET_RULES, 1, crc32fast::hash(&json), &tampered);

      let result: Result<TestEnvelope, _> = decode(TAG_BUDGET_RULES, 1, &pack_raw(&frame));

      assert!(matches!(result, Err(DecodeError::ChecksumMismatch { .. })));
    }

    #[test]
    fn it_rejects_a_frame_truncated_inside_the_header() {
      let json = serde_json::to_vec(&sample_envelope()).unwrap();
      let frame = build_frame(TAG_BUDGET_RULES, 1, crc32fast::hash(&json), &json);
      let truncated = &frame[..MAGIC.len() + 4];

      let result: Result<TestEnvelope, _> = decode(TAG_BUDGET_RULES, 1, &pack_raw(truncated));

      assert!(matches!(result, Err(DecodeError::Truncated)));
    }

    #[test]
    fn it_rejects_a_frame_cut_to_only_the_magic() {
      let result: Result<TestEnvelope, _> = decode(TAG_BUDGET_RULES, 1, &pack_raw(&MAGIC));

      assert!(matches!(result, Err(DecodeError::Truncated)));
    }

    #[test]
    fn it_rejects_a_truncated_encoded_string() {
      let encoded = encode(TAG_BUDGET_RULES, 1, &sample_envelope()).unwrap();
      let truncated = &encoded[..encoded.len() / 2];

      let result: Result<TestEnvelope, _> = decode(TAG_BUDGET_RULES, 1, truncated);

      assert!(result.is_err());
    }

    #[test]
    fn it_rejects_malformed_json_with_a_valid_checksum() {
      let body = b"definitely not json";
      let frame = build_frame(TAG_BUDGET_RULES, 1, crc32fast::hash(body), body);

      let result: Result<TestEnvelope, _> = decode(TAG_BUDGET_RULES, 1, &pack_raw(&frame));

      assert!(matches!(result, Err(DecodeError::Json(_))));
    }

    #[test]
    fn it_rejects_empty_input() {
      let result: Result<TestEnvelope, _> = decode(TAG_BUDGET_RULES, 1, "");

      assert!(result.is_err());
    }
  }

  mod encode {
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn it_produces_an_opaque_body() {
      let encoded = encode(TAG_BUDGET_RULES, 1, &sample_envelope()).unwrap();

      assert!(!encoded.contains("Corp starter rules"));
      assert!(!encoded.contains(TAG_BUDGET_RULES));
      assert!(STANDARD.decode(&encoded).is_ok());
    }

    #[test]
    fn it_is_deterministic_for_the_same_envelope() {
      let first = encode(TAG_BUDGET_RULES, 1, &sample_envelope()).unwrap();
      let second = encode(TAG_BUDGET_RULES, 1, &sample_envelope()).unwrap();

      assert_eq!(first, second);
    }

    #[test]
    fn it_rejects_a_tag_longer_than_a_byte_length() {
      let tag = "x".repeat(300);

      let result = encode(&tag, 1, &sample_envelope());

      assert!(matches!(result, Err(EncodeError::TagTooLong(300))));
    }
  }
}
