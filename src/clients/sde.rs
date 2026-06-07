use std::{
  collections::HashMap,
  io::{Read as _, Write as _},
  path::{Path, PathBuf},
  sync::Arc,
};

use tempfile::TempDir;

use crate::clients::{self, http};

const SDE_YAML_URL: &str = "https://developers.eveonline.com/static-data/eve-online-static-data-latest-yaml.zip";

pub struct Client {
  url: String,
  http: Arc<http::Client>,
}

impl Client {
  pub fn new(http: Arc<http::Client>) -> Self {
    Self {
      url: SDE_YAML_URL.to_owned(),
      http,
    }
  }

  pub async fn download_and_extract(&self) -> Result<Sde, clients::Error> {
    let bytes = self.http.get_bytes_uncached(&self.url).await?;
    let temp_dir = tempfile::tempdir().map_err(|e| clients::Error::Internal(e.to_string()))?;
    let extract_dir = temp_dir.path().to_owned();
    extract_zip(bytes, extract_dir.clone()).await?;
    let root = find_sde_root(&extract_dir).await;
    let build_version = read_sde_build_version(&root).await;
    Ok(Sde {
      root,
      build_version,
      _temp_dir: temp_dir,
    })
  }

  #[cfg(test)]
  pub fn with_url(http: Arc<http::Client>, url: impl Into<String>) -> Self {
    Self {
      url: url.into(),
      http,
    }
  }
}

pub struct Sde {
  pub build_version: Option<String>,
  pub root: PathBuf,
  _temp_dir: TempDir,
}

fn extract_archive_entry<R: std::io::Read + std::io::Seek>(
  archive: &mut zip::ZipArchive<R>,
  index: usize,
  dest: &Path,
) -> Result<(), clients::Error> {
  let mut entry = archive
    .by_index(index)
    .map_err(|e| clients::Error::Internal(e.to_string()))?;
  let out_path = dest.join(entry.name());
  extract_zip_entry(&mut entry, &out_path)
}

fn extract_build_number(build: &serde_yaml::Value) -> Option<String> {
  match build {
    serde_yaml::Value::String(s) => Some(s.clone()),
    serde_yaml::Value::Number(n) => Some(n.to_string()),
    other => serde_yaml::to_string(other).ok().map(|s| s.trim().to_string()),
  }
}

async fn extract_zip(bytes: Vec<u8>, dest: PathBuf) -> Result<(), clients::Error> {
  tokio::task::spawn_blocking(move || extract_zip_sync(&bytes, &dest))
    .await
    .map_err(|e| clients::Error::Internal(e.to_string()))?
}

fn extract_zip_entry<R: std::io::Read + ?Sized>(
  entry: &mut zip::read::ZipFile<'_, R>,
  out_path: &Path,
) -> Result<(), clients::Error> {
  if entry.is_dir() {
    std::fs::create_dir_all(out_path).map_err(|e| clients::Error::Internal(e.to_string()))?;
  } else {
    write_zip_file_entry(entry, out_path)?;
  }
  Ok(())
}

fn extract_zip_sync(bytes: &[u8], dest: &Path) -> Result<(), clients::Error> {
  let reader = std::io::Cursor::new(bytes);
  let mut archive = zip::ZipArchive::new(reader).map_err(|e| clients::Error::Internal(e.to_string()))?;
  for i in 0..archive.len() {
    extract_archive_entry(&mut archive, i, dest)?;
  }
  Ok(())
}

async fn find_sde_root(extract_dir: &Path) -> PathBuf {
  if extract_dir.join("categories.yaml").exists() {
    return extract_dir.to_owned();
  }
  find_sde_root_in_subdirs(extract_dir)
    .await
    .unwrap_or_else(|| extract_dir.to_owned())
}

async fn find_sde_root_in_subdirs(extract_dir: &Path) -> Option<PathBuf> {
  let mut rd = tokio::fs::read_dir(extract_dir).await.ok()?;
  while let Ok(Some(entry)) = rd.next_entry().await {
    let path = entry.path();
    if path.is_dir() && path.join("categories.yaml").exists() {
      return Some(path);
    }
  }
  None
}

fn parse_sde_build_from_yaml(data: &str) -> Option<String> {
  let map: HashMap<String, serde_yaml::Value> = serde_yaml::from_str(data).ok()?;
  let sde = map.get("sde")?.as_mapping()?;
  let build = sde.get("buildNumber")?;
  extract_build_number(build)
}

async fn read_sde_build_version(root: &Path) -> Option<String> {
  let data = tokio::fs::read_to_string(root.join("_sde.yaml")).await.ok()?;
  parse_sde_build_from_yaml(&data)
}

fn write_zip_file_entry<R: std::io::Read + ?Sized>(
  entry: &mut zip::read::ZipFile<'_, R>,
  out_path: &Path,
) -> Result<(), clients::Error> {
  if let Some(parent) = out_path.parent() {
    std::fs::create_dir_all(parent).map_err(|e| clients::Error::Internal(e.to_string()))?;
  }
  let mut buf = Vec::new();
  entry
    .read_to_end(&mut buf)
    .map_err(|e| clients::Error::Internal(e.to_string()))?;
  let mut out = std::fs::File::create(out_path).map_err(|e| clients::Error::Internal(e.to_string()))?;
  out.write_all(&buf).map_err(|e| clients::Error::Internal(e.to_string()))
}

#[cfg(test)]
mod tests {
  use super::*;

  fn build_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
    let mut buf = Vec::new();
    {
      let cursor = std::io::Cursor::new(&mut buf);
      let mut writer = zip::ZipWriter::new(cursor);
      let opts: zip::write::FileOptions<'_, ()> = zip::write::FileOptions::default();
      for (name, data) in entries {
        zip::write::ZipWriter::start_file(&mut writer, *name, opts).unwrap();
        writer.write_all(data).unwrap();
      }
      writer.finish().unwrap();
    }
    buf
  }

  mod extract_zip {
    use super::*;

    #[tokio::test]
    async fn it_extracts_files_and_finds_the_sde_root() {
      let zip = build_zip(&[
        ("categories.yaml", b"6: {name: {en: Ship}}\n"),
        ("_sde.yaml", b"sde:\n  buildNumber: 12345\n"),
      ]);
      let dir = tempfile::tempdir().unwrap();
      let dest = dir.path().to_owned();

      extract_zip(zip, dest.clone()).await.unwrap();
      let root = find_sde_root(&dest).await;

      assert!(root.join("categories.yaml").exists());
      assert_eq!(root, dest);
    }

    #[tokio::test]
    async fn it_finds_the_sde_root_inside_a_wrapping_top_level_directory() {
      let zip = build_zip(&[
        ("sde/categories.yaml", b"6: {name: {en: Ship}}\n"),
        ("sde/_sde.yaml", b"sde:\n  buildNumber: 12345\n"),
      ]);
      let dir = tempfile::tempdir().unwrap();
      let dest = dir.path().to_owned();

      extract_zip(zip, dest.clone()).await.unwrap();
      let root = find_sde_root(&dest).await;

      assert!(root.join("categories.yaml").exists());
      assert_eq!(root, dest.join("sde"));

      let build = read_sde_build_version(&root).await;
      assert_eq!(build.as_deref(), Some("12345"));
    }
  }

  mod download_and_extract {
    use pretty_assertions::assert_eq;
    use wiremock::{
      Mock, MockServer, ResponseTemplate,
      matchers::{method, path},
    };

    use super::*;
    use crate::store;

    #[tokio::test]
    async fn it_downloads_extracts_and_resolves_the_root_and_build_version() {
      let zip = build_zip(&[
        ("categories.yaml", b"6: {name: {en: Ship}}\n"),
        ("_sde.yaml", b"sde:\n  buildNumber: 98765\n"),
      ]);
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/sde.zip"))
        .respond_with(ResponseTemplate::new(200).set_body_raw(zip, "application/zip"))
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      let http = http::Client::builder(http::Cache::new(db)).build();
      let client = Client::with_url(http, format!("{}/sde.zip", server.uri()));

      let sde = client.download_and_extract().await.unwrap();

      assert!(sde.root.join("categories.yaml").exists());
      assert_eq!(sde.build_version.as_deref(), Some("98765"));
    }

    #[tokio::test]
    async fn it_errors_when_the_download_fails() {
      let server = MockServer::start().await;
      Mock::given(method("GET"))
        .and(path("/missing.zip"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
      let db = store::open_test().await.unwrap();
      let http = http::Client::builder(http::Cache::new(db)).build();
      let client = Client::with_url(http, format!("{}/missing.zip", server.uri()));

      assert!(client.download_and_extract().await.is_err());
    }
  }

  mod parse_sde_build_from_yaml {
    use super::*;

    #[test]
    fn it_reads_a_numeric_build_number() {
      let yaml = "sde:\n  buildNumber: 20240101\n";

      assert_eq!(parse_sde_build_from_yaml(yaml).as_deref(), Some("20240101"));
    }

    #[test]
    fn it_reads_a_string_build_number() {
      let yaml = "sde:\n  buildNumber: \"20240101.1\"\n";

      assert_eq!(parse_sde_build_from_yaml(yaml).as_deref(), Some("20240101.1"));
    }

    #[test]
    fn it_returns_none_when_the_build_number_is_missing() {
      let yaml = "sde:\n  other: 1\n";

      assert_eq!(parse_sde_build_from_yaml(yaml), None);
    }
  }
}
