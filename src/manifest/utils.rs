use std::path::PathBuf;

use kdl::{KdlNode, NodeKey};

pub trait KdlUtils<K> {
  /// Fetches an entry by key and tries to map to a [PathBuf].
  fn get_pathbuf(&self, key: K) -> Option<PathBuf>;

  /// Fetches an entry by key and tries to map to a [String].
  fn get_string(&self, key: K) -> Option<String>;

  /// Fetches an entry by key and tries to map it to a [bool].
  fn get_bool(&self, key: K) -> Option<bool>;
}

impl<K> KdlUtils<K> for KdlNode
where
  K: Into<NodeKey>,
{
  fn get_pathbuf(&self, key: K) -> Option<PathBuf> {
    self
      .get(key)
      .and_then(|entry| entry.value().as_string().map(PathBuf::from))
  }

  fn get_string(&self, key: K) -> Option<String> {
    self
      .get(key)
      .and_then(|entry| entry.value().as_string().map(str::to_string))
  }

  fn get_bool(&self, key: K) -> Option<bool> {
    self.get(key).and_then(|entry| entry.value().as_bool())
  }
}
