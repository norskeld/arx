use std::path::PathBuf;

use glob_match::glob_match_with_captures;
use thiserror::Error;
use walkdir::{DirEntry, IntoIter as WalkDirIter, WalkDir};

#[derive(Debug, Error)]
pub enum TraverseError {
  #[error("Could not read entry while traversing directory.")]
  InvalidEntry(walkdir::Error),
}

#[derive(Debug)]
pub struct Match {
  /// Full path.
  pub path: PathBuf,
  /// Captured path relative to the traverser's root.
  pub captured: PathBuf,
  /// Original entry.
  pub entry: DirEntry,
}

#[derive(Debug)]
pub struct TraverseOptions {
  /// Directory to traverse.
  root: PathBuf,
  /// Pattern to match the path against. If `None`, all paths will match.
  pattern: Option<String>,
  /// Whether to ignore directories (not threir contents) when traversing. Defaults to `false`.
  ignore_dirs: bool,
  /// Whether to traverse contents of directories first (depth-first). Defaults to `false`.
  contents_first: bool,
}

#[derive(Debug)]
pub struct Traverser {
  /// Traverser options.
  options: TraverseOptions,
}

impl Traverser {
  /// Creates a new (consuming) builder.
  pub fn new<P: Into<PathBuf>>(root: P) -> Self {
    Self {
      options: TraverseOptions {
        root: root.into(),
        pattern: None,
        ignore_dirs: false,
        contents_first: false,
      },
    }
  }

  /// Set the pattern to match the path against.
  pub fn pattern(mut self, pattern: &str) -> Self {
    self.options.pattern = Some(pattern.to_string());
    self
  }

  /// Set whether to ignore directories (not their contents) when traversing or not.
  pub fn ignore_dirs(mut self, ignore_dirs: bool) -> Self {
    self.options.ignore_dirs = ignore_dirs;
    self
  }

  /// Set whether to traverse contents of directories first or not.
  pub fn contents_first(mut self, contents_first: bool) -> Self {
    self.options.contents_first = contents_first;
    self
  }

  /// Creates an iterator without consuming the traverser builder.
  pub fn iter(&self) -> TraverserIterator {
    let it = WalkDir::new(&self.options.root)
      .contents_first(self.options.contents_first)
      .into_iter();

    let root_pattern = self
      .options
      .pattern
      .as_ref()
      .map(|pat| self.options.root.join(pat).display().to_string());

    TraverserIterator { it, root_pattern, options: &self.options }
  }
}

/// Traverser iterator.
pub struct TraverserIterator<'t> {
  /// Inner iterator (using [walkdir::IntoIter]) that is used to do actual traversing.
  it: WalkDirIter,
  /// Pattern prepended with the root path to avoid conversions on every iteration.
  root_pattern: Option<String>,
  /// Traverser options.
  options: &'t TraverseOptions,
}

impl<'t> Iterator for TraverserIterator<'t> {
  type Item = Result<Match, TraverseError>;

  fn next(&mut self) -> Option<Self::Item> {
    let mut item = self.it.next()?;

    'skip: loop {
      match item {
        | Ok(entry) => {
          let path = entry.path();

          // This ignores only _entry_, while still stepping into the directory.
          if self.options.ignore_dirs && entry.file_type().is_dir() {
            item = self.it.next()?;

            continue 'skip;
          }

          if let Some(pattern) = &self.root_pattern {
            let candidate = path.display().to_string();

            if let Some(captures) = glob_match_with_captures(pattern, &candidate) {
              let range = captures.first().cloned().unwrap_or_default();
              let captured = PathBuf::from(&candidate[range.start..]);

              return Some(Ok(Match {
                path: path.to_path_buf(),
                captured,
                entry,
              }));
            }

            item = self.it.next()?;

            continue 'skip;
          }

          return Some(Ok(Match {
            path: path.to_path_buf(),
            captured: path.to_path_buf(),
            entry,
          }));
        },
        | Err(err) => return Some(Err(TraverseError::InvalidEntry(err))),
      }
    }
  }
}
