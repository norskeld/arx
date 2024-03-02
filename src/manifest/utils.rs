use kdl::{KdlNode, NodeKey};

pub trait KdlUtils<K> {
  /// Gets an entry by key and tries to map to a [String].
  fn get_string(&self, key: K) -> Option<String>;

  /// Gets an entry by key and tries to map it to a [bool].
  fn get_bool(&self, key: K) -> Option<bool>;
}

impl<K> KdlUtils<K> for KdlNode
where
  K: Into<NodeKey>,
{
  fn get_string(&self, key: K) -> Option<String> {
    self
      .get(key)
      .and_then(|entry| entry.value().as_string().map(str::to_string))
  }

  fn get_bool(&self, key: K) -> Option<bool> {
    self.get(key).and_then(|entry| entry.value().as_bool())
  }
}
