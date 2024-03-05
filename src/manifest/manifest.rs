use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use kdl::{KdlDocument, KdlNode};
use thiserror::Error;

use crate::manifest::actions::*;
use crate::manifest::prompts::*;
use crate::manifest::KdlUtils;

const MANIFEST_NAME: &str = "arx.kdl";

#[derive(Debug, Error)]
pub enum ManifestError {
  #[error("Couldn't read the manifest.")]
  ReadFail(#[from] io::Error),
  #[error("Couldn't parse the manifest.")]
  ParseFail(#[from] kdl::KdlError),
  #[error("You can use either suites of actions or a flat list of single actions, not both.")]
  MixedActions,
  #[error("Unknown node '{0}'.")]
  UnknownNode(String),
  #[error("Unknown prompt '{0}'.")]
  UnknownPrompt(String),
  #[error("Expected a suite name.")]
  ExpectedSuiteName,
  #[error("Expected attribute '{0}' to be present and not empty.")]
  ExpectedAttribute(String),
  #[error("Expected argument.")]
  ExpectedArgument,
  #[error("Expected argument for node '{0}'.")]
  ExpectedArgumentFor(String),
  #[error("Input prompt must have defined name and hint.")]
  ExpectedInputNodes,
  #[error("Editor prompt must have defined name and hint.")]
  ExpectedEditorNodes,
  #[error("Select prompt must have defined name, hint and variants.")]
  ExpectedSelectNodes,
  #[error("Confirm prompt must have defined name and hint.")]
  ExpectedConfirmNodes,
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
  /// Manifest options.
  pub options: ManifestOptions,
  /// Actions.
  pub actions: Actions,
}

impl Manifest {
  /// Creates a new manifest from the given path and options.
  pub fn with_options(path: &Path) -> Self {
    Self {
      root: path.to_path_buf(),
      options: ManifestOptions::default(),
      actions: Actions::Empty,
    }
  }

  /// Tries to load and parse the manifest.
  pub fn load(&mut self) -> Result<(), ManifestError> {
    if self.exists() {
      let doc = self.parse()?;
      self.options = self.get_options(&doc)?;
      self.actions = self.get_actions(&doc)?;
    }

    Ok(())
  }

  /// Checks if the manifest exists under `self.root`.
  fn exists(&self) -> bool {
    // TODO: Allow to override the config name.
    let file = self.root.join(MANIFEST_NAME);
    let file_exists = file.try_exists();

    file_exists.is_ok()
  }

  /// Reads and parses the manifest into a [KdlDocument].
  fn parse(&self) -> Result<KdlDocument, ManifestError> {
    let filename = self.root.join(MANIFEST_NAME);

    let contents = fs::read_to_string(filename).map_err(ManifestError::ReadFail)?;
    let document = contents.parse().map_err(ManifestError::ParseFail)?;

    Ok(document)
  }

  /// Tries to parse options from the manifest.
  fn get_options(&self, doc: &KdlDocument) -> Result<ManifestOptions, ManifestError> {
    let options = doc
      .get("options")
      .and_then(KdlNode::children)
      .map(|children| {
        let nodes = children.nodes();
        let mut defaults = ManifestOptions::default();

        for node in nodes {
          let name = node.name().to_string().to_ascii_lowercase();

          match name.as_str() {
            | "delete" => {
              defaults.delete = node
                .get_bool(0)
                .ok_or(ManifestError::ExpectedArgumentFor("delete".into()))?;
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
  fn get_actions(&self, doc: &KdlDocument) -> Result<Actions, ManifestError> {
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
          Err(ManifestError::MixedActions)
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
    let name = node.get_string(0).ok_or(ManifestError::ExpectedSuiteName)?;

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
        let from = node
          .get_string("from")
          .ok_or(ManifestError::ExpectedAttribute("from".into()))?;

        let to = node
          .get_string("to")
          .ok_or(ManifestError::ExpectedAttribute("to".into()))?;

        let overwrite = node.get_bool("overwrite").unwrap_or(true);

        ActionSingle::Copy(Copy { from, to, overwrite })
      },
      | "mv" => {
        let from = node
          .get_string("from")
          .ok_or(ManifestError::ExpectedAttribute("from".into()))?;

        let to = node
          .get_string("to")
          .ok_or(ManifestError::ExpectedAttribute("to".into()))?;

        let overwrite = node.get_bool("overwrite").unwrap_or(true);

        ActionSingle::Move(Move { from, to, overwrite })
      },
      | "rm" => {
        let target = node.get_string(0).ok_or(ManifestError::ExpectedArgument)?;

        ActionSingle::Delete(Delete { target })
      },
      // Running commands and echoing output.
      | "echo" => {
        let message = node
          .get_string(0)
          .ok_or(ManifestError::ExpectedAttribute("message".into()))?;

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
        let command = node.get_string(0).ok_or(ManifestError::ExpectedArgument)?;

        let injects = node.children().map(|children| {
          children
            .get_args("inject")
            .into_iter()
            .filter_map(|arg| arg.as_string().map(str::to_string))
            .collect()
        });

        ActionSingle::Run(Run { name, command, injects })
      },
      // Prompts and replacements.
      | "prompt" => {
        let prompt = self.get_prompt(node)?;

        ActionSingle::Prompt(prompt)
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
      | action => ActionSingle::Unknown(Unknown { name: action.into() }),
    };

    Ok(action)
  }

  fn get_prompt(&self, node: &KdlNode) -> Result<Prompt, ManifestError> {
    // Prompt kind, defaults to "input".
    let kind = node
      .get_string(0)
      .unwrap_or("input".into())
      .to_ascii_lowercase();

    #[inline]
    fn name(nodes: &KdlDocument) -> Result<String, ManifestError> {
      nodes
        .get("name")
        .and_then(|node| node.get_string(0))
        .ok_or(ManifestError::ExpectedArgumentFor("name".into()))
    }

    #[inline]
    fn hint(nodes: &KdlDocument) -> Result<String, ManifestError> {
      nodes
        .get("hint")
        .and_then(|node| node.get_string(0))
        .ok_or(ManifestError::ExpectedArgumentFor("hint".into()))
    }

    #[inline]
    fn options(nodes: &KdlDocument) -> Vec<String> {
      nodes
        .get_args("options")
        .into_iter()
        .filter_map(|arg| arg.as_string().map(str::to_string))
        .collect()
    }

    // Depending on the type construct a prompt.
    match kind.as_str() {
      | "input" => {
        let nodes = node.children().ok_or(ManifestError::ExpectedInputNodes)?;
        let default = nodes.get("default").and_then(|node| node.get_string(0));

        Ok(Prompt::Input(Input {
          name: name(nodes)?,
          hint: hint(nodes)?,
          default,
        }))
      },
      | "editor" => {
        let nodes = node.children().ok_or(ManifestError::ExpectedEditorNodes)?;
        let default = nodes.get("default").and_then(|node| node.get_string(0));

        Ok(Prompt::Editor(Editor {
          name: name(nodes)?,
          hint: hint(nodes)?,
          default,
        }))
      },
      | "select" => {
        let nodes = node.children().ok_or(ManifestError::ExpectedSelectNodes)?;

        Ok(Prompt::Select(Select {
          name: name(nodes)?,
          hint: hint(nodes)?,
          options: options(nodes),
        }))
      },
      | "confirm" => {
        let nodes = node.children().ok_or(ManifestError::ExpectedConfirmNodes)?;
        let default = nodes.get("default").and_then(|node| node.get_bool(0));

        Ok(Prompt::Confirm(Confirm {
          name: name(nodes)?,
          hint: hint(nodes)?,
          default,
        }))
      },
      | kind => Err(ManifestError::UnknownPrompt(kind.into())),
    }
  }
}
