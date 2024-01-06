use std::path::{Path, PathBuf};

use crate::path::expand;

pub trait PathUtils {
  /// Given `root`, returns `root` if `self` is `.`, otherwise returns `self`.
  fn to_root<P: AsRef<Path>>(&self, root: P) -> PathBuf;

  /// Expands tilde and environment variables in given `path`.
  fn expand(&self) -> PathBuf;
}

impl PathUtils for Path {
  fn to_root<P: AsRef<Path>>(&self, root: P) -> PathBuf {
    if self == Path::new(".") {
      root.as_ref().to_path_buf()
    } else {
      self.to_path_buf()
    }
  }

  fn expand(&self) -> PathBuf {
    let path = self.display().to_string();
    let expanded = expand(&path);

    PathBuf::from(expanded)
  }
}
