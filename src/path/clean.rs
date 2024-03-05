use std::path::{Component, Path, PathBuf};

/// Implements the [clean] method.
pub trait PathClean {
  fn clean(&self) -> PathBuf;
}

/// [PathClean] implemented for [Path].
impl PathClean for Path {
  fn clean(&self) -> PathBuf {
    clean(self)
  }
}

/// Cleans up a [Path].
///
/// It performs the following, lexically:
///
/// - Reduces multiple slashes to a single slash.
/// - Eliminates `.` path name elements (the current directory).
/// - Eliminates `..` path name elements (the parent directory) and the non-`.` non-`..`, element
///   that precedes them.
/// - Eliminates `..` elements that begin a rooted path, that is, replace `/..` by `/` at the
///   beginning of a path.
/// - Leaves intact `..` elements that begin a non-rooted path.
///
/// If the result is an empty string, returns the string `"."`, representing the current directory.
pub fn clean<P>(path: P) -> PathBuf
where
  P: AsRef<Path>,
{
  let mut out = Vec::new();

  for component in path.as_ref().components() {
    match component {
      | Component::CurDir => (),
      | Component::ParentDir => {
        match out.last() {
          | Some(Component::RootDir) => (),
          | Some(Component::Normal(_)) => {
            out.pop();
          },
          | None
          | Some(Component::CurDir)
          | Some(Component::ParentDir)
          | Some(Component::Prefix(_)) => out.push(component),
        }
      },
      | comp => out.push(comp),
    }
  }

  if !out.is_empty() {
    out.iter().collect()
  } else {
    PathBuf::from(".")
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  // Helpers.

  fn test_cases(cases: Vec<(&str, &str)>) {
    for (actual, expected) in cases {
      assert_eq!(clean(actual), PathBuf::from(expected));
    }
  }

  // Tests.

  #[test]
  fn test_trait() {
    assert_eq!(
      PathBuf::from("/test/../path/").clean(),
      PathBuf::from("/path")
    );

    assert_eq!(Path::new("/test/../path/").clean(), PathBuf::from("/path"));
  }

  #[test]
  fn test_empty_path_is_current_dir() {
    assert_eq!(clean(""), PathBuf::from("."));
  }

  #[test]
  fn test_clean_paths_dont_change() {
    let cases = vec![(".", "."), ("..", ".."), ("/", "/")];

    test_cases(cases);
  }

  #[test]
  fn test_replace_multiple_slashes() {
    let cases = vec![
      ("/", "/"),
      ("//", "/"),
      ("///", "/"),
      (".//", "."),
      ("//..", "/"),
      ("..//", ".."),
      ("/..//", "/"),
      ("/.//./", "/"),
      ("././/./", "."),
      ("path//to///thing", "path/to/thing"),
      ("/path//to///thing", "/path/to/thing"),
    ];

    test_cases(cases);
  }

  #[test]
  fn test_eliminate_current_dir() {
    let cases = vec![
      ("./", "."),
      ("/./", "/"),
      ("./test", "test"),
      ("./test/./path", "test/path"),
      ("/test/./path/", "/test/path"),
      ("test/path/.", "test/path"),
    ];

    test_cases(cases);
  }

  #[test]
  fn test_eliminate_parent_dir() {
    let cases = vec![
      ("/..", "/"),
      ("/../test", "/test"),
      ("test/..", "."),
      ("test/path/..", "test"),
      ("test/../path", "path"),
      ("/test/../path", "/path"),
      ("test/path/../../", "."),
      ("test/path/../../..", ".."),
      ("/test/path/../../..", "/"),
      ("/test/path/../../../..", "/"),
      ("test/path/../../../..", "../.."),
      ("test/path/../../another/path", "another/path"),
      ("test/path/../../another/path/..", "another"),
      ("../test", "../test"),
      ("../test/", "../test"),
      ("../test/path", "../test/path"),
      ("../test/..", ".."),
    ];

    test_cases(cases);
  }

  #[test]
  #[cfg(windows)]
  fn test_windows_paths() {
    let cases = vec![
      ("\\..", "\\"),
      ("\\..\\test", "\\test"),
      ("test\\..", "."),
      ("test\\path\\..\\..\\..", ".."),
      ("test\\path/..\\../another\\path", "another\\path"), // Mixed
      ("/dir\\../otherDir/test.json", "/otherDir/test.json"), // User example
    ];

    test_cases(cases);
  }
}
