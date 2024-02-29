use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use git2::build::CheckoutBuilder;
use git2::Repository as GitRepository;
use thiserror::Error;

use crate::fs::Traverser;
use crate::path::PathUtils;

#[derive(Debug, Error, PartialEq)]
pub enum ParseError {
  #[error("Host must be one of: github/gh, gitlab/gl, or bitbucket/bb.")]
  InvalidHost,
  #[error("Invalid user name. Only ASCII alphanumeric characters, _ and - allowed.")]
  InvalidUserName,
  #[error("Invalid repository name. Only ASCII alphanumeric characters, _, - and . allowed.")]
  InvalidRepositoryName,
  #[error("Missing repository name.")]
  MissingRepositoryName,
  #[error("Multiple / in the input.")]
  MultipleSlash,
}

#[derive(Debug, Error, PartialEq)]
pub enum FetchError {
  #[error("Request failed.")]
  RequestFailed,
  #[error("Repository download ({0}) failed with code {1}.")]
  RequestFailedWithCode(String, u16),
  #[error("Couldn't get the response body as bytes.")]
  RequestBodyFailed,
}

#[derive(Debug, Error)]
pub enum CopyError {
  #[error("Failed to create directory.")]
  CreateDirFailed(io::Error),
  #[error("Failed to copy file.")]
  CopyFailed(io::Error),
}

#[derive(Debug, Error)]
pub enum CheckoutError {
  #[error("Failed to open the git repository.")]
  OpenFailed(git2::Error),
  #[error("Failed to parse revision string '{0}'.")]
  RevparseFailed(String),
  #[error("Failed to checkout revision (tree).")]
  TreeCheckoutFailed,
  #[error("Reference name is not a valid UTF-8 string.")]
  InvalidRefName,
  #[error("Failed to set HEAD to '{0}'.")]
  SetHeadFailed(String),
  #[error("Failed to detach HEAD to '{0}'.")]
  DetachHeadFailed(String),
}

/// Supported hosts. [GitHub][RepositoryHost::GitHub] is the default one.
#[derive(Debug, Default, PartialEq)]
pub enum RepositoryHost {
  #[default]
  GitHub,
  GitLab,
  BitBucket,
}

/// Container for a repository host.
#[derive(Debug)]
pub enum Host {
  Known(RepositoryHost),
  Unknown,
}

impl Default for Host {
  fn default() -> Self {
    Host::Known(RepositoryHost::default())
  }
}

/// Repository meta or *ref*, i.e. branch, tag or commit.
///
/// This newtype exists solely for providing the default value.
#[derive(Clone, Debug, PartialEq)]
pub struct RepositoryMeta(pub String);

impl Default for RepositoryMeta {
  fn default() -> Self {
    // Using "HEAD" instead of hardcoding the default branch name like "master" or "main".
    // Suprisingly, works just fine.
    RepositoryMeta("HEAD".to_string())
  }
}

/// Represents a remote repository. Repositories of this kind need to be downloaded first.
#[derive(Debug, PartialEq)]
pub struct RemoteRepository {
  pub host: RepositoryHost,
  pub user: String,
  pub repo: String,
  pub meta: RepositoryMeta,
}

impl RemoteRepository {
  /// Returns a list of valid host prefixes.
  pub fn prefixes() -> Vec<String> {
    vec!["github", "gh", "gitlab", "gl", "bitbucket", "bb"]
      .into_iter()
      .map(str::to_string)
      .collect()
  }

  /// Resolves a URL depending on the host and other repository fields.
  pub fn get_tar_url(&self) -> String {
    let RemoteRepository {
      host,
      user,
      repo,
      meta,
    } = self;

    let RepositoryMeta(meta) = meta;

    match host {
      | RepositoryHost::GitHub => {
        format!("https://github.com/{user}/{repo}/archive/{meta}.tar.gz")
      },
      | RepositoryHost::GitLab => {
        format!("https://gitlab.com/{user}/{repo}/-/archive/{meta}/{repo}.tar.gz")
      },
      | RepositoryHost::BitBucket => {
        format!("https://bitbucket.org/{user}/{repo}/get/{meta}.tar.gz")
      },
    }
  }

  /// Fetches the tarball using the resolved URL, and reads it into bytes (`Vec<u8>`).
  pub async fn fetch(&self) -> Result<Vec<u8>, FetchError> {
    let url = self.get_tar_url();

    let response = reqwest::get(&url).await.map_err(|err| {
      err.status().map_or(FetchError::RequestFailed, |status| {
        FetchError::RequestFailedWithCode(url.clone(), status.as_u16())
      })
    })?;

    let status = response.status();

    if !status.is_success() {
      return Err(FetchError::RequestFailedWithCode(url, status.as_u16()));
    }

    response
      .bytes()
      .await
      .map(|bytes| bytes.to_vec())
      .map_err(|_| FetchError::RequestBodyFailed)
  }
}

/// Represents a local repository. Repositories of this kind don't need to be downloaded, we can
/// simply clone them locally and switch to desired meta (ref).
#[derive(Debug, PartialEq)]
pub struct LocalRepository {
  pub source: PathBuf,
  pub meta: RepositoryMeta,
}

impl LocalRepository {
  /// Returns a list of valid prefixes that can be used to identify local repositories.
  pub fn prefixes() -> [&'static str; 2] {
    ["file", "local"]
  }

  /// Copies the repository into the `destination` directory.
  pub fn copy(&self, destination: &Path) -> Result<(), CopyError> {
    let root = self.source.expand();

    let traverser = Traverser::new(root)
      .pattern("**/*")
      .ignore_dirs(true)
      .contents_first(true);

    for matched in traverser.iter().flatten() {
      let target = destination.join(&matched.captured);

      if let Some(parent) = target.parent() {
        fs::create_dir_all(parent).map_err(CopyError::CreateDirFailed)?;
        fs::copy(matched.path, &target).map_err(CopyError::CopyFailed)?;
      }
    }

    Ok(())
  }

  /// Checks out the repository located at the `destination`.
  pub fn checkout(&self, destination: &Path) -> Result<(), CheckoutError> {
    let RepositoryMeta(meta) = &self.meta;

    // First, try to create Repository.
    let repository = GitRepository::open(destination).map_err(CheckoutError::OpenFailed)?;

    // Note: in case of local repositories, instead of HEAD we want to check origin/HEAD first,
    // which should be the default branch if the repository has been cloned from a remote.
    // Otherwise we fallback to HEAD, which will point to whatever the repository points at the time
    // of cloning (can be absolutely arbitrary reference/state).
    let meta = if meta == "HEAD" {
      repository
        .revparse_ext("origin/HEAD")
        .ok()
        .and_then(|(_, reference)| reference)
        .and_then(|reference| reference.name().map(str::to_string))
        .unwrap_or("HEAD".to_string())
    } else {
      "HEAD".to_string()
    };

    // Try to find (parse revision) the desired reference: branch, tag or commit. They are encoded
    // in two objects:
    //
    // - `object` contains (among other things) the commit hash.
    // - `reference` points to the branch or tag.
    let (object, reference) = repository
      .revparse_ext(&meta)
      .map_err(|_| CheckoutError::RevparseFailed(meta))?;

    // Build checkout options.
    let mut checkout = CheckoutBuilder::new();

    checkout
      .skip_unmerged(true)
      .remove_untracked(true)
      .remove_ignored(true)
      .force();

    // Updates files in the index and working tree.
    repository
      .checkout_tree(&object, Some(&mut checkout))
      .map_err(|_| CheckoutError::TreeCheckoutFailed)?;

    match reference {
      // Here `gref`` is an actual reference like branch or tag.
      | Some(gref) => {
        let ref_name = gref.name().ok_or(CheckoutError::InvalidRefName)?;

        repository
          .set_head(ref_name)
          .map_err(|_| CheckoutError::SetHeadFailed(ref_name.to_string()))?;
      },
      // This is a commit, detach HEAD.
      | None => {
        let hash = object.id();

        repository
          .set_head_detached(hash)
          .map_err(|_| CheckoutError::DetachHeadFailed(hash.to_string()))?;
      },
    }

    Ok(())
  }
}

/// Wrapper around `RemoteRepository` and `LocalRepository`.
#[derive(Debug, PartialEq)]
pub enum Repository {
  Remote(RemoteRepository),
  Local(LocalRepository),
}

impl Repository {
  /// Returns a new `Repository` with the given `meta`.
  pub fn with_meta(self, meta: RepositoryMeta) -> Self {
    match self {
      | Self::Remote(remote) => Self::Remote(RemoteRepository { meta, ..remote }),
      | Self::Local(local) => Self::Local(LocalRepository { meta, ..local }),
    }
  }

  /// Returns a copy of the `Repository`'s `meta`.
  pub fn meta(&self) -> RepositoryMeta {
    match self {
      | Self::Remote(remote) => remote.meta.clone(),
      | Self::Local(local) => local.meta.clone(),
    }
  }
}

impl FromStr for Repository {
  type Err = ParseError;

  /// Parses a `&str` into a `Repository`.
  fn from_str(input: &str) -> Result<Self, Self::Err> {
    #[inline(always)]
    fn is_valid_user(ch: char) -> bool {
      ch.is_ascii_alphanumeric() || ch == '_' || ch == '-'
    }

    #[inline(always)]
    fn is_valid_repo(ch: char) -> bool {
      is_valid_user(ch) || ch == '.'
    }

    // Try to find and remove a local repository prefix. If we get Some(..), we are facing a local
    // repository, otherwise a remote one.
    let unprefix = LocalRepository::prefixes()
      .into_iter()
      .map(|prefix| format!("{prefix}:"))
      .find_map(|prefix| input.strip_prefix(&prefix));

    if let Some(input) = unprefix {
      Ok(Repository::Local(LocalRepository {
        source: PathBuf::from(input),
        meta: RepositoryMeta::default(),
      }))
    } else {
      // TODO: Handle an edge case with multuple slashes in the repository name.

      let input = input.trim();

      // Parse host if present or use default otherwise.
      let (host, input) = if let Some((host, rest)) = input.split_once(':') {
        match host.to_ascii_lowercase().as_str() {
          | "github" | "gh" => (RepositoryHost::GitHub, rest),
          | "gitlab" | "gl" => (RepositoryHost::GitLab, rest),
          | "bitbucket" | "bb" => (RepositoryHost::BitBucket, rest),
          | _ => return Err(ParseError::InvalidHost),
        }
      } else {
        (RepositoryHost::default(), input)
      };

      // Parse user name.
      let (user, input) = if let Some((user, rest)) = input.split_once('/') {
        if user.chars().all(is_valid_user) {
          (user.to_string(), rest)
        } else {
          return Err(ParseError::InvalidUserName);
        }
      } else {
        return Err(ParseError::MissingRepositoryName);
      };

      // Parse repository name.
      let (repo, input) = if let Some((repo, rest)) = input.split_once('#') {
        if repo.chars().all(is_valid_repo) {
          (repo.to_string(), Some(rest))
        } else {
          return Err(ParseError::InvalidRepositoryName);
        }
      } else {
        (input.to_string(), None)
      };

      // Produce meta if anything left from the input. Empty meta is accepted but ignored, default
      // value is used then.
      let meta = input
        .filter(|input| !input.is_empty())
        .map_or(RepositoryMeta::default(), |input| {
          RepositoryMeta(input.to_string())
        });

      Ok(Repository::Remote(RemoteRepository {
        host,
        user,
        repo,
        meta,
      }))
    }
  }
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn parse_remote_default() {
    assert_eq!(
      Repository::from_str("foo/bar"),
      Ok(Repository::Remote(RemoteRepository {
        host: RepositoryHost::GitHub,
        user: "foo".to_string(),
        repo: "bar".to_string(),
        meta: RepositoryMeta::default()
      }))
    );
  }

  #[test]
  fn parse_remote_invalid_userrepo() {
    assert_eq!(
      Repository::from_str("foo-bar"),
      Err(ParseError::MissingRepositoryName)
    );
  }

  #[test]
  fn parse_remote_invalid_host() {
    assert_eq!(
      Repository::from_str("srht:foo/bar"),
      Err(ParseError::InvalidHost)
    );
  }

  #[test]
  fn parse_remote_meta() {
    let cases = [
      ("foo/bar", RepositoryMeta::default()),
      ("foo/bar#foo", RepositoryMeta("foo".to_string())),
      ("foo/bar#4a5a56fd", RepositoryMeta("4a5a56fd".to_string())),
      (
        "foo/bar#feat/some-feature-name",
        RepositoryMeta("feat/some-feature-name".to_string()),
      ),
    ];

    for (input, meta) in cases {
      assert_eq!(
        Repository::from_str(input),
        Ok(Repository::Remote(RemoteRepository {
          host: RepositoryHost::GitHub,
          user: "foo".to_string(),
          repo: "bar".to_string(),
          meta
        }))
      );
    }
  }

  #[test]
  fn parse_remote_hosts() {
    let cases = [
      ("github:foo/bar", RepositoryHost::GitHub),
      ("gh:foo/bar", RepositoryHost::GitHub),
      ("gitlab:foo/bar", RepositoryHost::GitLab),
      ("gl:foo/bar", RepositoryHost::GitLab),
      ("bitbucket:foo/bar", RepositoryHost::BitBucket),
      ("bb:foo/bar", RepositoryHost::BitBucket),
    ];

    for (input, host) in cases {
      assert_eq!(
        Repository::from_str(input),
        Ok(Repository::Remote(RemoteRepository {
          host,
          user: "foo".to_string(),
          repo: "bar".to_string(),
          meta: RepositoryMeta::default()
        }))
      );
    }
  }

  #[test]
  fn test_remote_empty_meta() {
    assert_eq!(
      Repository::from_str("foo/bar#"),
      Ok(Repository::Remote(RemoteRepository {
        host: RepositoryHost::GitHub,
        user: "foo".to_string(),
        repo: "bar".to_string(),
        meta: RepositoryMeta::default()
      }))
    );
  }

  #[test]
  fn parse_remote_ambiguous_username() {
    let cases = [
      ("github/foo", "github", "foo"),
      ("gh/foo", "gh", "foo"),
      ("gitlab/foo", "gitlab", "foo"),
      ("gl/foo", "gl", "foo"),
      ("bitbucket/foo", "bitbucket", "foo"),
      ("bb/foo", "bb", "foo"),
    ];

    for (input, user, repo) in cases {
      assert_eq!(
        Repository::from_str(input),
        Ok(Repository::Remote(RemoteRepository {
          host: RepositoryHost::default(),
          user: user.to_string(),
          repo: repo.to_string(),
          meta: RepositoryMeta::default()
        }))
      );
    }
  }

  #[test]
  fn parse_local() {
    assert_eq!(
      Repository::from_str("file:~/dev/templates"),
      Ok(Repository::Local(LocalRepository {
        source: PathBuf::from("~/dev/templates"),
        meta: RepositoryMeta::default()
      }))
    );
  }
}
