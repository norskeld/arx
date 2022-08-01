use chumsky::error::Cheap;
use chumsky::prelude::*;

use crate::app::AppError;
use crate::repository::{Host, Repository, RepositoryHost, RepositoryMeta};

type ParseResult = (Option<Host>, (String, String), Option<RepositoryMeta>);

/// Parses source argument of the following form:
///
/// `({host}:){user}/{repo}(#{branch|commit|tag})`.
pub(crate) fn shortcut(input: &str) -> Result<Repository, AppError> {
  let host = host().or_not();
  let meta = meta().or_not().then_ignore(end());
  let repo = repository().then(meta);

  let shortcut = host.then(repo).map(|(a, (b, c))| (a, b, c)).parse(input);

  match shortcut {
    | Ok(data) => produce_result(data),
    | Err(error) => Err(produce_error(error)),
  }
}

/// Parses the repository host. Must be one of:
/// - `github` or `gh`
/// - `gitlab` or `gl`
/// - `bitbucket` or `bb`
fn host() -> impl Parser<char, Host, Error = Cheap<char>> {
  let host = filter::<_, _, Cheap<char>>(|ch: &char| ch.is_ascii_alphabetic())
    .repeated()
    .at_least(1)
    .collect::<String>()
    .map(|variant| {
      match variant.as_str() {
        | "github" | "gh" => Host::Known(RepositoryHost::GitHub),
        | "gitlab" | "gl" => Host::Known(RepositoryHost::GitLab),
        | "bitbucket" | "bb" => Host::Known(RepositoryHost::BitBucket),
        | _ => Host::Unknown,
      }
    })
    .labelled("Host can't be zero-length.");

  host.then_ignore(just(':'))
}

/// Parses the user name and repository name.
fn repository() -> impl Parser<char, (String, String), Error = Cheap<char>> {
  fn is_valid_user(ch: &char) -> bool {
    ch.is_ascii_alphanumeric() || ch == &'_' || ch == &'-'
  }

  fn is_valid_repo(ch: &char) -> bool {
    is_valid_user(ch) || ch == &'.'
  }

  let user = filter::<_, _, Cheap<char>>(is_valid_user)
    .repeated()
    .at_least(1)
    .labelled("Must be a valid user name. Allowed symbols: [a-zA-Z0-9_-]")
    .collect::<String>();

  let repo = filter::<_, _, Cheap<char>>(is_valid_repo)
    .repeated()
    .at_least(1)
    .labelled("Must be a valid repository name. Allowed symbols: [a-zA-Z0-9_-.]")
    .collect::<String>();

  user
    .then_ignore(
      just('/').labelled("There must be a slash between the user name and the repository name."),
    )
    .then(repo)
}

/// Parses the shortcut meta (branch, commit hash, or tag), which may be specified after `#`.
///
/// TODO: Add some loose validation.
fn meta() -> impl Parser<char, RepositoryMeta, Error = Cheap<char>> {
  let meta = filter::<_, _, Cheap<char>>(char::is_ascii)
    .repeated()
    .at_least(1)
    .labelled("Meta can't be zero-length.")
    .collect::<String>()
    .map(RepositoryMeta);

  just('#').ignore_then(meta)
}

fn produce_result(data: ParseResult) -> Result<Repository, AppError> {
  match data {
    | (host, (user, repo), meta) => {
      let meta = meta.unwrap_or_default();
      let host = host.unwrap_or_default();

      if let Host::Known(host) = host {
        Ok(Repository {
          host,
          user,
          repo,
          meta,
        })
      } else {
        Err(AppError(
          "Host must be one of: github/gh, gitlab/gl, or bitbucket/bb.".to_string(),
        ))
      }
    },
  }
}

fn produce_error(errors: Vec<Cheap<char>>) -> AppError {
  let reduced = errors
    .iter()
    .filter_map(|error| error.label())
    .map(str::to_string)
    .collect::<Vec<String>>();

  AppError(reduced.join("\n"))
}
