use crate::app::AppError;

/// Supported hosts. [GitHub][RepositoryHost::GitHub] is the default one.
#[derive(Debug)]
pub(crate) enum RepositoryHost {
  GitHub,
  GitLab,
  BitBucket,
}

impl Default for RepositoryHost {
  fn default() -> Self {
    RepositoryHost::GitHub
  }
}

/// Container for a repository host.
#[derive(Debug)]
pub(crate) enum Host {
  Known(RepositoryHost),
  Unknown,
}

impl Default for Host {
  fn default() -> Self {
    Host::Known(RepositoryHost::default())
  }
}

/// Repository meta, i.e. *ref*.
///
/// This newtype exists solely for providing the default value.
#[derive(Debug)]
pub(crate) struct RepositoryMeta(pub String);

impl Default for RepositoryMeta {
  fn default() -> Self {
    // Using "HEAD" instead of hardcoding the default branch name like "master" or "main".
    // Suprisingly, works just fine.
    RepositoryMeta("HEAD".to_string())
  }
}

#[derive(Debug)]
pub(crate) struct Repository {
  pub host: RepositoryHost,
  pub user: String,
  pub repo: String,
  pub meta: RepositoryMeta,
}

impl Repository {
  /// Resolves a URL depending on the host and other repository fields.
  pub(crate) fn get_tar_url(&self) -> String {
    let Repository {
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
  pub(crate) async fn fetch(&self) -> Result<Vec<u8>, AppError> {
    let url = self.get_tar_url();

    let response = reqwest::get(url).await.map_err(|err| {
      err
        .status()
        .map_or(AppError("Request failed.".to_string()), |status| {
          AppError(format!(
            "Request failed with the code: {code}.",
            code = status.as_u16()
          ))
        })
    })?;

    response
      .bytes()
      .await
      .map(|bytes| bytes.to_vec())
      .map_err(|_| AppError("Couldn't get the response body as bytes.".to_string()))
  }
}
