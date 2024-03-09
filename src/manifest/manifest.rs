use std::fs;
use std::io::Error as IoError;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use kdl::{KdlDocument, KdlError, KdlNode};
use miette::{Diagnostic, LabeledSpan, NamedSource, Report};
use thiserror::Error;

use crate::manifest::actions::*;
use crate::manifest::prompts::*;
use crate::manifest::KdlUtils;

const MANIFEST_NAME: &str = "arx.kdl";

/// Helper macro to create a [ManifestError::Diagnostic] in a slightly less verbose way.
macro_rules! diagnostic {
  ($source:ident = $code:expr, $($key:ident = $value:expr,)* $fmt:literal $($arg:tt)*) => {
    ManifestError::Diagnostic(
      miette::Report::from(
        miette::diagnostic!($($key = $value,)* $fmt $($arg)*)
      ).with_source_code(Arc::clone($code))
    )
  };
}

#[derive(Debug, Diagnostic, Error)]
pub enum ManifestError {
  #[error("{message}")]
  #[diagnostic(code(arx::manifest::io))]
  Io {
    message: String,
    #[source]
    source: IoError,
  },

  #[error(transparent)]
  #[diagnostic(transparent)]
  Kdl(KdlError),

  #[error("{0}")]
  #[diagnostic(transparent)]
  Diagnostic(Report),
}

/// Manifest options. These may be overriden from the CLI.
#[derive(Debug)]
pub struct ManifestOptions {
  /// Whether to delete the manifest after we (successfully) done running.
  pub delete: bool,
}

impl Default for ManifestOptions {
  fn default() -> Self {
    Self { delete: true }
  }
}

/// Manifest options that may override parsed options.
#[derive(Debug, Default)]
pub struct ManifestOptionsOverrides {
  /// Whether to delete the manifest after we (successfully) done running.
  pub delete: Option<bool>,
}

/// Represents a manifest actions set that can be a vec of [ActionSuite] *or* [ActionSingle].
///
/// So, actions should be defined either like this:
///
/// ```kdl
/// actions {
///   suite "suite-one" { ... }
///   suite "suite-two" { ... }
///   ...
/// }
/// ```
///
/// Or like this:
///
/// ```kdl
/// actions {
///   cp from="..." to="..."
///   mv from="..." to="..."
///   ...
/// }
/// ```
#[derive(Debug)]
pub enum Actions {
  Suite(Vec<ActionSuite>),
  Flat(Vec<ActionSingle>),
  Empty,
}

/// A suite of actions that contains a flat list of [ActionSingle].
#[derive(Debug)]
pub struct ActionSuite {
  /// Suite name.
  pub name: String,
  /// Suite actions to run.
  pub actions: Vec<ActionSingle>,
}

/// A single "atomic" action.
#[derive(Debug)]
pub enum ActionSingle {
  /// Copies a file or directory. Glob-friendly. Overwrites by default.
  Copy(Copy),
  /// Moves a file or directory. Glob-friendly. Overwrites by default.
  Move(Move),
  /// Deletes a file or directory. Glob-friendly.
  Delete(Delete),
  /// Echoes a message to stdout.
  Echo(Echo),
  /// Runs an arbitrary command in the shell.
  Run(Run),
  /// Executes a prompt asking a declaratively defined "question".
  Prompt(Prompt),
  /// Execute given replacements using values provided by prompts. Optionally, only apply
  /// replacements to files matching the provided glob.
  Replace(Replace),
  /// Fallback action for pattern matching ergonomics and reporting purposes.
  Unknown(Unknown),
}

/// Arx manifest (config).
#[derive(Debug)]
pub struct Manifest {
  /// Manifest directory.
  pub root: PathBuf,
  /// Source. Wrapped in an [Arc] for cheap clones.
  pub source: Arc<NamedSource>,
  /// Manifest file path.
  pub config: PathBuf,
  /// Manifest options.
  pub options: ManifestOptions,
  /// Actions.
  pub actions: Actions,
}

impl Manifest {
  /// Creates a new manifest from the given path and options.
  pub fn new(root: &Path) -> Self {
    let root = root.to_path_buf();
    let config = root.join(MANIFEST_NAME);

    // NOTE: Creating dummy source first, will be overwritten with actual data on load. This is done
    // because of some limitations around `NamedSource` and related entities like `SourceCode` which
    // I couldn't figure out.
    let source = Arc::new(NamedSource::new(
      config.display().to_string(),
      String::default(),
    ));

    Self {
      config,
      options: ManifestOptions::default(),
      actions: Actions::Empty,
      source,
      root,
    }
  }

  /// Tries to apply the given overrides to the manifest options.
  pub fn override_with(&mut self, overrides: ManifestOptionsOverrides) {
    if let Some(delete) = overrides.delete {
      self.options.delete = delete;
    }
  }

  /// Tries to load and parse the manifest.
  pub fn load(&mut self) -> Result<(), ManifestError> {
    if self.exists() {
      let doc = self.parse()?;
      self.options = self.get_manifest_options(&doc)?;
      self.actions = self.get_manifest_actions(&doc)?;
    }

    Ok(())
  }

  /// Checks if the manifest exists under `self.root`.
  fn exists(&self) -> bool {
    self.config.try_exists().unwrap_or(false)
  }

  /// Reads and parses the manifest into a [KdlDocument].
  fn parse(&mut self) -> Result<KdlDocument, ManifestError> {
    let filename = self.root.join(MANIFEST_NAME);

    let contents = fs::read_to_string(&filename).map_err(|source| {
      ManifestError::Io {
        message: "Failed to read the manifest.".to_string(),
        source,
      }
    })?;

    let document = contents.parse().map_err(ManifestError::Kdl)?;

    // Replace dummy source with actual data.
    self.source = Arc::new(NamedSource::new(filename.display().to_string(), contents));

    Ok(document)
  }

  /// Tries to parse options from the manifest.
  fn get_manifest_options(&self, doc: &KdlDocument) -> Result<ManifestOptions, ManifestError> {
    let options = doc
      .get("options")
      .and_then(KdlNode::children)
      .map(|children| {
        let nodes = children.nodes();
        let mut defaults = ManifestOptions::default();

        for node in nodes {
          let option = node.name().to_string().to_ascii_lowercase();

          match option.as_str() {
            | "delete" => {
              defaults.delete = node.get_bool(0).ok_or_else(|| {
                diagnostic!(
                  source = &self.source,
                  code = "arx::manifest::options",
                  labels = vec![LabeledSpan::at(
                    node.span().to_owned(),
                    "this node requires a boolean argument"
                  )],
                  "Missing required argument."
                )
              })?;
            },
            | _ => {
              continue;
            },
          }
        }

        Ok(defaults)
      });

    match options {
      | Some(Ok(options)) => Ok(options),
      | Some(Err(err)) => Err(err),
      | None => Ok(ManifestOptions::default()),
    }
  }

  /// Tries to parse actions from the manifest.
  fn get_manifest_actions(&self, doc: &KdlDocument) -> Result<Actions, ManifestError> {
    #[inline]
    fn is_suite(node: &KdlNode) -> bool {
      node.name().value() == "suite"
    }

    #[inline]
    fn is_flat(node: &KdlNode) -> bool {
      !is_suite(node)
    }

    let actions = doc
      .get("actions")
      .and_then(KdlNode::children)
      .map(|children| {
        let nodes = children.nodes();

        // Check if all nodes are suites.
        if nodes.iter().all(is_suite) {
          let mut suites = Vec::new();

          for node in nodes.iter() {
            let suite = self.get_action_suite(node)?;
            suites.push(suite);
          }

          Ok(Actions::Suite(suites))
        }
        // Check if all nodes are single actions.
        else if nodes.iter().all(is_flat) {
          let mut actions = Vec::new();

          for node in nodes.iter() {
            let action = self.get_action_single(node)?;
            actions.push(action);
          }

          Ok(Actions::Flat(actions))
        }
        // Otherwise we have invalid actions block.
        else {
          Err(ManifestError::Diagnostic(miette::miette!(
            code = "arx::manifest::actions",
            "You can use either suites of actions or a flat list of single actions, not both."
          )))
        }
      });

    match actions {
      | Some(Ok(action)) => Ok(action),
      | Some(Err(err)) => Err(err),
      | None => Ok(Actions::Empty),
    }
  }

  fn get_action_suite(&self, node: &KdlNode) -> Result<ActionSuite, ManifestError> {
    let mut actions = Vec::new();

    // Fail if we stumbled upon a nameless suite.
    let name = self.get_arg_string(node)?;

    if let Some(children) = node.children() {
      for children in children.nodes() {
        let action = self.get_action_single(children)?;
        actions.push(action);
      }
    }

    Ok(ActionSuite { name, actions })
  }

  fn get_action_single(&self, node: &KdlNode) -> Result<ActionSingle, ManifestError> {
    let kind = node.name().to_string().to_ascii_lowercase();

    let action = match kind.as_str() {
      // Actions for manipulating files and directories.
      | "cp" => {
        ActionSingle::Copy(Copy {
          from: self.get_attr_string(node, "from")?,
          to: self.get_attr_string(node, "to")?,
          overwrite: node.get_bool("overwrite").unwrap_or(true),
        })
      },
      | "mv" => {
        ActionSingle::Move(Move {
          from: self.get_attr_string(node, "from")?,
          to: self.get_attr_string(node, "to")?,
          overwrite: node.get_bool("overwrite").unwrap_or(true),
        })
      },
      | "rm" => ActionSingle::Delete(Delete { target: self.get_arg_string(node)? }),
      // Actions for running commands and echoing output.
      | "echo" => {
        let message = self.get_arg_string(node)?;

        // TODO: Verify injects have valid type (string values).
        let injects = node.children().map(|children| {
          children
            .get_args("inject")
            .into_iter()
            .filter_map(|arg| arg.as_string().map(str::to_string))
            .collect()
        });

        let trim = node.get_bool("trim").unwrap_or(true);

        ActionSingle::Echo(Echo { message, injects, trim })
      },
      | "run" => {
        let name = node.get_string("name");
        let command = self.get_arg_string(node)?;

        let injects = node.children().map(|children| {
          children
            .get_args("inject")
            .into_iter()
            .filter_map(|arg| arg.as_string().map(str::to_string))
            .collect()
        });

        ActionSingle::Run(Run { name, command, injects })
      },
      // Actions for prompts and replacements.
      | "input" => {
        let nodes = self.get_children(node, vec!["hint"])?;

        ActionSingle::Prompt(Prompt::Input(Input {
          name: self.get_arg_string(node)?,
          hint: self.get_hint(node, nodes)?,
          default: self.get_default_string(nodes),
        }))
      },
      | "editor" => {
        let nodes = self.get_children(node, vec!["hint"])?;

        ActionSingle::Prompt(Prompt::Editor(Editor {
          name: self.get_arg_string(node)?,
          hint: self.get_hint(node, nodes)?,
          default: self.get_default_string(nodes),
        }))
      },
      | "select" => {
        let nodes = self.get_children(node, vec!["hint", "options"])?;

        ActionSingle::Prompt(Prompt::Select(Select {
          name: self.get_arg_string(node)?,
          hint: self.get_hint(node, nodes)?,
          options: self.get_options(node, nodes)?,
          default: self.get_default_string(nodes),
        }))
      },
      | "confirm" => {
        let nodes = self.get_children(node, vec!["hint"])?;

        ActionSingle::Prompt(Prompt::Confirm(Confirm {
          name: self.get_arg_string(node)?,
          hint: self.get_hint(node, nodes)?,
          default: self.get_default_bool(nodes),
        }))
      },
      | "replace" => {
        let replacements = node
          .children()
          .map(|children| {
            children
              .nodes()
              .iter()
              .map(|node| node.name().value().to_string())
              .collect()
          })
          .unwrap_or_default();

        let glob = node.get_string("in");

        ActionSingle::Replace(Replace { replacements, glob })
      },
      // Fallback.
      | action => ActionSingle::Unknown(Unknown { name: action.to_string() }),
    };

    Ok(action)
  }

  fn get_arg_string(&self, node: &KdlNode) -> Result<String, ManifestError> {
    let start = node.span().offset();
    let end = start + node.name().len();

    node.get_string(0).ok_or_else(|| {
      diagnostic!(
        source = &self.source,
        code = "arx::manifest::actions",
        labels = vec![
          LabeledSpan::at(start..end, "this node requires a string argument"),
          LabeledSpan::at_offset(end, "argument should be here")
        ],
        "Missing required argument."
      )
    })
  }

  fn get_attr_string(&self, node: &KdlNode, key: &str) -> Result<String, ManifestError> {
    node.get_string(key).ok_or_else(|| {
      diagnostic!(
        source = &self.source,
        code = "arx::manifest::actions",
        labels = vec![LabeledSpan::at(
          node.span().to_owned(),
          format!("this node requires the `{key}` attribute")
        )],
        "Missing required attribute: `{key}`."
      )
    })
  }

  fn get_children<'kdl>(
    &self,
    node: &'kdl KdlNode,
    nodes: Vec<&str>,
  ) -> Result<&'kdl KdlDocument, ManifestError> {
    let suffix = if nodes.len() > 1 { "s" } else { "" };
    let nodes = nodes
      .iter()
      .map(|node| format!("`{node}`"))
      .collect::<Vec<_>>()
      .join(", ");

    let message = format!("Missing required child node{suffix}: {nodes}.");

    node.children().ok_or_else(|| {
      diagnostic!(
        source = &self.source,
        code = "arx::manifest::actions",
        labels = vec![LabeledSpan::at(
          node.span().to_owned(),
          format!("this node requires the following child nodes: {nodes}")
        )],
        "{message}"
      )
    })
  }

  fn get_hint(&self, parent: &KdlNode, nodes: &KdlDocument) -> Result<String, ManifestError> {
    let hint = nodes.get("hint").ok_or_else(|| {
      diagnostic!(
        source = &self.source,
        code = "arx::manifest::actions",
        labels = vec![LabeledSpan::at(
          parent.span().to_owned(),
          "prompts require a `hint` child node"
        )],
        "Missing prompt hint."
      )
    })?;

    self.get_arg_string(hint)
  }

  fn get_options(
    &self,
    parent: &KdlNode,
    nodes: &KdlDocument,
  ) -> Result<Vec<String>, ManifestError> {
    let options = nodes.get("options").ok_or_else(|| {
      diagnostic!(
        source = &self.source,
        code = "arx::manifest::actions",
        labels = vec![LabeledSpan::at(
          parent.span().to_owned(),
          "select prompts require the `options` child node"
        )],
        "Missing select prompt options."
      )
    })?;

    let mut variants = Vec::new();

    for entry in options.entries() {
      let value = entry.value();
      let span = entry.span().to_owned();

      let value = if value.is_float_value() {
        value.as_f64().as_ref().map(f64::to_string)
      } else if value.is_i64_value() {
        value.as_i64().as_ref().map(i64::to_string)
      } else if value.is_string_value() {
        value.as_string().map(str::to_string)
      } else {
        return Err(diagnostic!(
          source = &self.source,
          code = "arx::manifest::actions",
          labels = vec![LabeledSpan::at(
            span,
            "option values can be either strings or numbers"
          )],
          "Invalid select option type."
        ));
      };

      let option = value.ok_or_else(|| {
        diagnostic!(
          source = &self.source,
          code = "arx::manifest::actions",
          labels = vec![LabeledSpan::at(
            span,
            "failed to converted this value to a string"
          )],
          "Failed to convert option value."
        )
      })?;

      variants.push(option);
    }

    Ok(variants)
  }

  fn get_default_string(&self, nodes: &KdlDocument) -> Option<String> {
    nodes.get("default").and_then(|node| node.get_string(0))
  }

  fn get_default_bool(&self, nodes: &KdlDocument) -> Option<bool> {
    nodes.get("default").and_then(|node| node.get_bool(0))
  }
}
