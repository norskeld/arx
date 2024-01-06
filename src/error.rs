use thiserror::Error;

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum ArxError {
  #[error("Host must be one of: github/gh, gitlab/gl, or bitbucket/bb.")]
  ShortcutInvalidHost,
  #[error("Host can't be zero-length.")]
  ShortcutEmptyHost,
  #[error("Expected colon after the host.")]
  ShortcutHostDelimiterRequired,
  #[error("Invalid user name. Allowed symbols: [a-zA-Z0-9_-].")]
  ShortcutInvalidUser,
  #[error("Invalid repository name. Allowed symbols: [a-zA-Z0-9_-.].")]
  ShortcutInvalidRepository,
  #[error("Both user and repository name must be specified.")]
  ShortcutUserRepositoryNameRequired,
  #[error("Meta can't be zero-length.")]
  ShortcutEmptyMeta,
}
