use std::cmp::Ordering;
use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use base32::Alphabet;
use chrono::{DateTime, Utc};
use crossterm::style::Stylize;
use miette::{Diagnostic, Report};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::repository::RemoteRepository;

/// Unpadded Base 32 alphabet.
const BASE32_ALPHABET: Alphabet = Alphabet::RFC4648 { padding: false };

/// `%userprofile%/AppData/Local/arx/.cache`
#[cfg(target_os = "windows")]
const CACHE_ROOT: &str = "AppData/Local/arx/.cache";

/// `$HOME/.cache/arx`
#[cfg(not(target_os = "windows"))]
const CACHE_ROOT: &str = ".cache/arx";

/// `<CACHE_ROOT>/tarballs/<hash>.tar.gz`
const CACHE_TARBALLS_DIR: &str = "tarballs";

/// `<CACHE_ROOT>/manifest.toml`
const CACHE_MANIFEST: &str = "manifest.toml";

#[derive(Debug, Diagnostic, Error)]
pub enum CacheError {
  #[error("{message}")]
  #[diagnostic(code(arx::cache::io))]
  Io {
    message: String,
    #[source]
    source: io::Error,
  },
  #[error(transparent)]
  #[diagnostic(code(arx::cache::manifest::serialize))]
  TomlSerialize(toml::ser::Error),
  #[error(transparent)]
  #[diagnostic(code(arx::cache::manifest::deserialize))]
  TomlDeserialize(toml::de::Error),
  #[error("{0}")]
  #[diagnostic(transparent)]
  Diagnostic(Report),
}

/// Entry name in the form of Base 32 encoded source string.
type Entry = String;

/// Cache manifest.
///
/// # Structure
///
/// ```toml
/// [[templates.<entry>.items]]
/// name = "<name>"
/// hash = "<hash>"
/// timestamp = <timestamp>
/// ```
///
/// Where:
///
/// - `<entry>` - Base 32 encoded source string in the form of: `<host>:<user>/<repo>`.
/// - `<name>` - Ref name or commit hash.
/// - `<hash>` - Ref/commit hash, either short of full. Used in filenames.
/// - `<timestamp>` - Unix timestamp in milliseconds.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Manifest {
  templates: HashMap<Entry, Template>,
}

/// Represents a template table.
#[derive(Debug, Serialize, Deserialize)]
pub struct Template {
  /// List of linked items in the template table.
  items: Vec<Item>,
}

/// Represents a linked item in the template table.
#[derive(Debug, Serialize, Deserialize)]
pub struct Item {
  /// Ref name or commit hash.
  name: String,
  /// Ref/commit hash, either short of full.
  hash: String,
  /// Unix timestamp in milliseconds.
  timestamp: i64,
}

#[derive(Debug)]
pub struct Cache {
  /// Root cache directory.
  root: PathBuf,
  /// Manifest.
  manifest: Manifest,
}

impl Cache {
  /// Initializes cache and creates manifest if it doesn't exist.
  pub fn init() -> miette::Result<Self> {
    let root = Self::get_root()?;
    let manifest = Self::read_manifest(&root)?;

    Ok(Self { root, manifest })
  }

  /// Returns the root cache directory.
  fn get_root() -> miette::Result<PathBuf> {
    home::home_dir()
      .map(|home| home.join(CACHE_ROOT))
      .ok_or(miette::miette!("Failed to resolve home directory."))
  }

  /// Checks if two hashes match.
  fn compare_hashes(left: &str, right: &str) -> bool {
    match left.len().cmp(&right.len()) {
      | Ordering::Less => right.starts_with(left),
      | Ordering::Greater => left.starts_with(right),
      | Ordering::Equal => left == right,
    }
  }

  /// Reads manifest from disk.
  fn read_manifest<P: AsRef<Path>>(root: P) -> miette::Result<Manifest> {
    let location = root.as_ref().join(CACHE_MANIFEST);

    if !location.is_file() {
      // If the manifest file does not exist, we do not return an error.
      return Ok(Manifest::default());
    }

    let contents = fs::read_to_string(&location).map_err(|source| {
      CacheError::Io {
        message: "Failed to read the manifest.".to_string(),
        source,
      }
    })?;

    let manifest = toml::from_str(&contents).map_err(CacheError::TomlDeserialize)?;

    Ok(manifest)
  }

  /// Writes manifest to disk.
  fn write_manifest(&mut self) -> miette::Result<()> {
    let manifest = toml::to_string(&self.manifest).map_err(CacheError::TomlSerialize)?;

    fs::write(self.root.join(CACHE_MANIFEST), manifest).map_err(|source| {
      CacheError::Io {
        message: "Failed to write the manifest to disk.".to_string(),
        source,
      }
    })?;

    Ok(())
  }

  /// Writes contents to cache.
  pub fn write(
    &mut self,
    source: &str,
    name: &str,
    hash: &str,
    contents: &[u8],
  ) -> miette::Result<()> {
    let entry = base32::encode(BASE32_ALPHABET, source.as_bytes());
    let timestamp = Utc::now().timestamp_millis();

    self
      .manifest
      .templates
      .entry(entry)
      .and_modify(|template| {
        let hash = hash.to_string();
        let name = name.to_string();

        if !template
          .items
          .iter()
          .any(|item| Self::compare_hashes(&hash, &item.hash))
        {
          template.items.push(Item { name, hash, timestamp });
        }
      })
      .or_insert_with(|| {
        Template {
          items: vec![Item {
            name: name.to_string(),
            hash: hash.to_string(),
            timestamp,
          }],
        }
      });

    self.write_manifest()?;

    let tarballs_dir = self.root.join(CACHE_TARBALLS_DIR);
    let tarball = tarballs_dir.join(format!("{hash}.tar.gz"));

    fs::create_dir_all(&tarballs_dir).map_err(|source| {
      CacheError::Io {
        message: format!("Failed to create the '{CACHE_TARBALLS_DIR}' directory."),
        source,
      }
    })?;

    fs::write(tarball, contents).map_err(|source| {
      CacheError::Io {
        message: "Failed to write the tarball contents to disk.".to_string(),
        source,
      }
    })?;

    Ok(())
  }

  /// Reads from cache.
  pub fn read(&self, source: &str, hash: &str) -> miette::Result<Option<Vec<u8>>> {
    let entry = base32::encode(BASE32_ALPHABET, source.as_bytes());

    if let Some(template) = self.manifest.templates.get(&entry) {
      let item = template
        .items
        .iter()
        .find(|item| Self::compare_hashes(hash, &item.hash));

      if let Some(item) = item {
        let tarball = self
          .root
          .join(CACHE_TARBALLS_DIR)
          .join(format!("{}.tar.gz", item.hash));

        let contents = fs::read(tarball).map_err(|source| {
          CacheError::Io {
            message: "Failed to read the cached tarball.".to_string(),
            source,
          }
        })?;

        return Ok(Some(contents));
      }
    }

    Ok(None)
  }

  /// Lists cache entries.
  pub fn list(&self) -> Result<(), CacheError> {
    for (key, template) in &self.manifest.templates {
      if let Some(bytes) = base32::decode(BASE32_ALPHABET, key) {
        let entry = String::from_utf8(bytes).map_err(|_| {
          CacheError::Diagnostic(miette::miette!(
            code = "arx::cache::invalid_utf8",
            help = "Manifest may be malformed, clear the cache and try again.",
            "Couldn't decode entry due to invalid UTF-8 in the string: `{key}`."
          ))
        })?;

        let repo = RemoteRepository::from_str(&entry).map_err(|_| {
          CacheError::Diagnostic(miette::miette!(
            code = "arx::cache::malformed_entry",
            help = "Manifest may be malformed, clear the cache and try again.",
            "Couldn't parse entry: `{key}`."
          ))
        })?;

        let host = repo.host.to_string().cyan();
        let name = format!("{}/{}", repo.user, repo.repo).green();

        println!("⋅ {host}:{name}");

        for item in &template.items {
          if let Some(date) = DateTime::from_timestamp_millis(item.timestamp) {
            let date = date.format("%d/%m/%Y %H:%M").to_string().dim();
            let name = item.name.clone().cyan();
            let hash = item.hash.clone().yellow();

            println!("└─ {date} @ {name} ╌╌ {hash}");
          }
        }
      } else {
        return Err(CacheError::Diagnostic(miette::miette!(
          code = "arx::cache::malformed_entry",
          help = "Manifest may be malformed, clear the cache and try again.",
          "Couldn't decode entry: `{key}`."
        )));
      }
    }

    Ok(())
  }

  /// Clears cache.
  pub fn clear(&mut self) -> miette::Result<()> {
    self.manifest.templates.clear();

    fs::remove_dir_all(self.root.join(CACHE_TARBALLS_DIR)).map_err(|source| {
      CacheError::Io {
        message: format!("Failed to clear the '{CACHE_TARBALLS_DIR}' directory."),
        source,
      }
    })?;

    self.write_manifest()?;

    Ok(())
  }
}
