use std::path::{Path, PathBuf};

pub trait PathUtils {
  /// Given `root`, returns `root` if `self` is `.`, otherwise returns `self`.
  fn to_root<P: AsRef<Path>>(&self, root: P) -> PathBuf;
}

impl PathUtils for Path {
  fn to_root<P: AsRef<Path>>(&self, root: P) -> PathBuf {
    if self == Path::new(".") {
      root.as_ref().to_path_buf()
    } else {
      self.to_path_buf()
    }
  }
}
