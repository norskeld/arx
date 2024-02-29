use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use kdl::{KdlDocument, KdlNode};
use thiserror::Error;

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

/// Represents a manifest actions set that can be either an [ActionSuite] *or* an [ActionSingle].
///
/// Actions should be defined either like this:
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
  Single(Vec<ActionSingle>),
  Empty,
}

/// A suite of actions that contains a flat list of [ActionSingle].
#[derive(Debug)]
pub struct ActionSuite {
  /// Suite name.
  pub name: String,
  /// Suite actions to run (synchronously).
  pub actions: Vec<ActionSingle>,
}

/// A single "atomic" action.
#[derive(Debug)]
pub enum ActionSingle {
  /// Copies a file or directory. Glob-friendly. Overwrites by default.
  Copy {
    /// Source(s) to copy.
    from: PathBuf,
    /// Where to copy to.
    to: PathBuf,
    /// Whether to overwrite or not.
    overwrite: bool,
  },
  /// Moves a file or directory. Glob-friendly. Overwrites by default.
  Move {
    /// Source(s) to move.
    from: PathBuf,
    /// Where to move to.
    to: PathBuf,
    /// Whether to overwrite or not.
    overwrite: bool,
  },
  /// Deletes a file or directory. Glob-friendly.
  Delete {
    /// Target to delete.
    target: PathBuf,
  },
  /// Simply outputs a message.
  Echo {
    /// Message to output.
    message: String,
    /// Whether to trim multiline message or not.
    trim: bool,
  },
  /// Runs an arbitrary command in the shell.
  Run {
    /// Command name. Optional, defaults either to the command itself or to the first line of
    /// the multiline command.
    name: Option<String>,
    /// Comannd to run in the shell.
    command: String,
    /// An optional list of replacements to be injected into the command. Consider the following
    /// example:
    ///
    /// We use inject to disambiguate whether `{R_PM}` is part of a command or is a replacement
    /// that should be replaced with something, we pass `inject` node that explicitly tells arx
    /// what to inject into the string.
    ///
    /// ```kdl
    /// run "{R_PM} install {R_PM_ARGS}" {
    ///   inject "R_PM" "R_PM_ARGS"
    /// }
    /// ```
    ///
    /// All replacements are processed _before_ running a command.
    inject: Option<Vec<String>>,
  },
  /// Executes a prompt asking a declaratively defined "question".
  Prompt(Prompt),
  /// Execute given replacements using values provided by prompts. Optionally, only apply
  /// replacements to files matching the provided glob.
  Replace {
    /// Replacements to apply.
    replacements: Vec<String>,
    /// Optional glob to limit files to apply replacements to.
    glob: Option<PathBuf>,
  },
  /// Fallback action for pattern matching ergonomics and reporting purposes.
  Unknown { name: String },
}

#[derive(Debug)]
pub enum Prompt {
  Input {
    /// Name of the variable that will store the answer.
    name: String,
    /// Short description.
    hint: String,
    /// Default value if input is empty.
    default: Option<String>,
  },
  Select {
    /// Name of the variable that will store the answer.
    name: String,
    /// Short description.
    hint: String,
    /// List of options.
    options: Vec<String>,
    /// Default value. If none or invalid option is provided, the first one is selected.
    default: Option<String>,
  },
  Confirm {
    /// Name of the variable that will store the answer.
    name: String,
    /// Short description of the prompt.
    hint: String,
    /// Default value.
    default: Option<bool>,
  },
  Editor {
    /// Name of the variable that will store the answer.
    name: String,
    /// Short description.
    hint: String,
    /// Default value if input is empty.
    default: Option<String>,
  },
}

/// Arx manifest (config).
#[derive(Debug)]
pub struct Manifest {
  /// Manifest directory.
  root: PathBuf,
  /// Manifest options.
  options: ManifestOptions,
  /// Actions.
  actions: Actions,
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
      let options = self.get_options(&doc)?;
      let actions = self.get_actions(&doc)?;

      println!("Options: {options:#?}");
      println!("Actions: {actions:#?}");

      self.options = options;
      self.actions = actions;
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

  fn get_actions(&self, doc: &KdlDocument) -> Result<Actions, ManifestError> {
    #[inline]
    fn is_suite(node: &KdlNode) -> bool {
      node.name().value() == "suite"
    }

    #[inline]
    fn is_not_suite(node: &KdlNode) -> bool {
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
        else if nodes.iter().all(is_not_suite) {
          let mut actions = Vec::new();

          for node in nodes.iter() {
            let action = self.get_action_single(node)?;
            actions.push(action);
          }

          Ok(Actions::Single(actions))
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
          .get_pathbuf("from")
          .ok_or(ManifestError::ExpectedAttribute("from".into()))?;

        let to = node
          .get_pathbuf("to")
          .ok_or(ManifestError::ExpectedAttribute("to".into()))?;

        let overwrite = node.get_bool("overwrite").unwrap_or(true);

        ActionSingle::Copy {
          from,
          to,
          overwrite,
        }
      },
      | "mv" => {
        let from = node
          .get_pathbuf("from")
          .ok_or(ManifestError::ExpectedAttribute("from".into()))?;

        let to = node
          .get_pathbuf("to")
          .ok_or(ManifestError::ExpectedAttribute("to".into()))?;

        let overwrite = node.get_bool("overwrite").unwrap_or(true);

        ActionSingle::Move {
          from,
          to,
          overwrite,
        }
      },
      | "rm" => {
        ActionSingle::Delete {
          target: node.get_pathbuf(0).ok_or(ManifestError::ExpectedArgument)?,
        }
      },
      // Running commands and echoing output.
      | "echo" => {
        let message = node
          .get_string(0)
          .ok_or(ManifestError::ExpectedAttribute("message".into()))?;

        let trim = node.get_bool("trim").unwrap_or(false);

        ActionSingle::Echo { message, trim }
      },
      | "run" => {
        let name = node.get_string("name");
        let command = node.get_string(0).ok_or(ManifestError::ExpectedArgument)?;

        let inject = node.children().map(|children| {
          children
            .get_args("inject")
            .into_iter()
            .filter_map(|arg| arg.as_string().map(str::to_string))
            .collect()
        });

        ActionSingle::Run {
          name,
          command,
          inject,
        }
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
              .collect::<Vec<_>>()
          })
          .unwrap_or_default();

        let glob = node.get_string("in").map(PathBuf::from);

        ActionSingle::Replace { replacements, glob }
      },
      // Fallback.
      | action => {
        return Err(ManifestError::UnknownNode(action.into()));
      },
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
    fn variants(nodes: &KdlDocument) -> Vec<String> {
      nodes
        .get_args("variants")
        .into_iter()
        .filter_map(|arg| arg.as_string().map(str::to_string))
        .collect()
    }

    // Depending on the type construct a prompt.
    match kind.as_str() {
      | "input" => {
        let nodes = node.children().ok_or(ManifestError::ExpectedInputNodes)?;

        Ok(Prompt::Input {
          name: name(nodes)?,
          hint: hint(nodes)?,
          default: nodes.get("default").and_then(|node| node.get_string(0)),
        })
      },
      | "editor" => {
        let nodes = node.children().ok_or(ManifestError::ExpectedEditorNodes)?;

        Ok(Prompt::Editor {
          name: name(nodes)?,
          hint: hint(nodes)?,
          default: nodes.get("default").and_then(|node| node.get_string(0)),
        })
      },
      | "select" => {
        let nodes = node.children().ok_or(ManifestError::ExpectedSelectNodes)?;

        Ok(Prompt::Select {
          name: name(nodes)?,
          hint: hint(nodes)?,
          options: variants(nodes),
          default: nodes.get("default").and_then(|node| node.get_string(0)),
        })
      },
      | "confirm" => {
        let nodes = node.children().ok_or(ManifestError::ExpectedConfirmNodes)?;

        Ok(Prompt::Confirm {
          name: name(nodes)?,
          hint: hint(nodes)?,
          default: nodes.get("default").and_then(|node| node.get_bool(0)),
        })
      },
      | kind => Err(ManifestError::UnknownPrompt(kind.into())),
    }
  }
}
