use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use kdl::{KdlDocument, KdlNode};
use miette::{Diagnostic, LabeledSpan, NamedSource, Report};
use thiserror::Error;

use crate::config::actions::*;
use crate::config::prompts::*;
use crate::config::value::*;
use crate::config::KdlUtils;

const CONFIG_NAME: &str = "decaff.kdl";

/// Helper macro to create a [ConfigError::Diagnostic] in a slightly less verbose way.
macro_rules! diagnostic {
  ($source:ident = $code:expr, $($key:ident = $value:expr,)* $fmt:literal $($arg:tt)*) => {
    ConfigError::Diagnostic(
      miette::Report::from(
        miette::diagnostic!($($key = $value,)* $fmt $($arg)*)
      ).with_source_code(Arc::clone($code))
    )
  };
}

#[derive(Debug, Diagnostic, Error)]
pub enum ConfigError {
  #[error("{message}")]
  #[diagnostic(code(decaff::config::io))]
  Io {
    message: String,
    #[source]
    source: io::Error,
  },
  #[error(transparent)]
  #[diagnostic(transparent)]
  Kdl(kdl::KdlError),
  #[error("{0}")]
  #[diagnostic(transparent)]
  Diagnostic(Report),
}

/// Config options. These may be overriden from the CLI.
#[derive(Debug)]
pub struct ConfigOptions {
  /// Whether to delete the config after we (successfully) done running.
  pub delete: bool,
}

impl Default for ConfigOptions {
  fn default() -> Self {
    Self { delete: true }
  }
}

/// Config options that may override parsed options.
#[derive(Debug, Default)]
pub struct ConfigOptionsOverrides {
  /// Whether to delete the config after we (successfully) done running.
  pub delete: Option<bool>,
}

/// Represents a config actions set that can be a vec of [ActionSuite] *or* [ActionSingle].
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
  /// Suites of actions to run.
  Suite(Vec<ActionSuite>),
  /// Flat list of actions to run.
  Flat(Vec<ActionSingle>),
  /// No actions to run.
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

/// decaff config.
#[derive(Debug)]
pub struct Config {
  /// Config directory.
  pub root: PathBuf,
  /// Source. Wrapped in an [Arc] for cheap clones.
  pub source: Arc<NamedSource>,
  /// Config file path.
  pub config: PathBuf,
  /// Config options.
  pub options: ConfigOptions,
  /// Actions.
  pub actions: Actions,
}

impl Config {
  /// Creates a new config from the given path and options.
  pub fn new(root: &Path) -> Self {
    let root = root.to_path_buf();
    let config = root.join(CONFIG_NAME);

    // NOTE: Creating dummy source first, will be overwritten with actual data on load. This is done
    // because of some limitations around `NamedSource` and related entities like `SourceCode` which
    // I couldn't figure out.
    let source = Arc::new(NamedSource::new(
      config.display().to_string(),
      String::default(),
    ));

    Self {
      config,
      options: ConfigOptions::default(),
      actions: Actions::Empty,
      source,
      root,
    }
  }

  /// Tries to apply the given overrides to the config options.
  pub fn override_with(&mut self, overrides: ConfigOptionsOverrides) {
    if let Some(delete) = overrides.delete {
      self.options.delete = delete;
    }
  }

  /// Tries to load and parse the config.
  pub fn load(&mut self) -> Result<bool, ConfigError> {
    if self.exists() {
      let doc = self.parse()?;
      self.options = self.get_config_options(&doc)?;
      self.actions = self.get_config_actions(&doc)?;

      Ok(true)
    } else {
      Ok(false)
    }
  }

  /// Checks if the config exists under `self.root`.
  fn exists(&self) -> bool {
    self.config.try_exists().unwrap_or(false)
  }

  /// Reads and parses the config into a [KdlDocument].
  fn parse(&mut self) -> Result<KdlDocument, ConfigError> {
    let filename = self.root.join(CONFIG_NAME);

    let contents = fs::read_to_string(&filename).map_err(|source| {
      ConfigError::Io {
        message: "Failed to read the config.".to_string(),
        source,
      }
    })?;

    let document = contents.parse().map_err(ConfigError::Kdl)?;

    // Replace dummy source with actual data.
    self.source = Arc::new(NamedSource::new(filename.display().to_string(), contents));

    Ok(document)
  }

  /// Tries to parse options from the config.
  fn get_config_options(&self, doc: &KdlDocument) -> Result<ConfigOptions, ConfigError> {
    let options = doc
      .get("options")
      .and_then(KdlNode::children)
      .map(|children| {
        let nodes = children.nodes();
        let mut defaults = ConfigOptions::default();

        for node in nodes {
          let option = node.name().to_string().to_ascii_lowercase();

          match option.as_str() {
            | "delete" => {
              defaults.delete = node.get_bool(0).ok_or_else(|| {
                diagnostic!(
                  source = &self.source,
                  code = "decaff::config::options",
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
      | None => Ok(ConfigOptions::default()),
    }
  }

  /// Tries to parse actions from the config.
  fn get_config_actions(&self, doc: &KdlDocument) -> Result<Actions, ConfigError> {
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
          Err(ConfigError::Diagnostic(miette::miette!(
            code = "decaff::config::actions",
            "You can use either suites of actions or a flat list of single actions. \
             Right now you have a mix of both."
          )))
        }
      });

    match actions {
      | Some(Ok(action)) => Ok(action),
      | Some(Err(err)) => Err(err),
      | None => Ok(Actions::Empty),
    }
  }

  fn get_action_suite(&self, node: &KdlNode) -> Result<ActionSuite, ConfigError> {
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

  fn get_action_single(&self, node: &KdlNode) -> Result<ActionSingle, ConfigError> {
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
        ActionSingle::Echo(Echo {
          message: self.get_arg_string(node)?,
          injects: self.get_injects(node),
          trim: node.get_bool("trim").unwrap_or(true),
        })
      },
      | "run" => {
        ActionSingle::Run(Run {
          name: node.get_string("name"),
          command: self.get_arg_string(node)?,
          injects: self.get_injects(node),
        })
      },
      // Actions for prompts and replacements.
      | "input" => {
        let nodes = self.get_children(node, vec!["hint"])?;

        ActionSingle::Prompt(Prompt::Input(InputPrompt {
          name: self.get_arg_string(node)?,
          hint: self.get_hint(node, nodes)?,
          default: self.get_default_string(nodes),
        }))
      },
      | "number" => {
        let nodes = self.get_children(node, vec!["hint"])?;

        ActionSingle::Prompt(Prompt::Number(NumberPrompt {
          name: self.get_arg_string(node)?,
          hint: self.get_hint(node, nodes)?,
          default: self.get_default_number(nodes),
        }))
      },
      | "editor" => {
        let nodes = self.get_children(node, vec!["hint"])?;

        ActionSingle::Prompt(Prompt::Editor(EditorPrompt {
          name: self.get_arg_string(node)?,
          hint: self.get_hint(node, nodes)?,
          default: self.get_default_string(nodes),
        }))
      },
      | "select" => {
        let nodes = self.get_children(node, vec!["hint", "options"])?;

        ActionSingle::Prompt(Prompt::Select(SelectPrompt {
          name: self.get_arg_string(node)?,
          hint: self.get_hint(node, nodes)?,
          options: self.get_options(node, nodes)?,
        }))
      },
      | "confirm" => {
        let nodes = self.get_children(node, vec!["hint"])?;

        ActionSingle::Prompt(Prompt::Confirm(ConfirmPrompt {
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

  fn get_arg_string(&self, node: &KdlNode) -> Result<String, ConfigError> {
    let start = node.span().offset();
    let end = start + node.name().len();

    node.get_string(0).ok_or_else(|| {
      diagnostic!(
        source = &self.source,
        code = "decaff::config::actions",
        labels = vec![
          LabeledSpan::at(start..end, "this node requires a string argument"),
          LabeledSpan::at_offset(end, "argument should be here")
        ],
        "Missing required argument."
      )
    })
  }

  fn get_attr_string(&self, node: &KdlNode, key: &str) -> Result<String, ConfigError> {
    node.get_string(key).ok_or_else(|| {
      diagnostic!(
        source = &self.source,
        code = "decaff::config::actions",
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
  ) -> Result<&'kdl KdlDocument, ConfigError> {
    let nodes = nodes
      .iter()
      .map(|node| format!("`{node}`"))
      .collect::<Vec<_>>()
      .join(", ");

    let suffix = if nodes.len() > 1 { "s" } else { "" };
    let message = format!("Missing required child node{suffix}: {nodes}.");

    node.children().ok_or_else(|| {
      diagnostic!(
        source = &self.source,
        code = "decaff::config::actions",
        labels = vec![LabeledSpan::at(
          node.span().to_owned(),
          format!("this node requires the following child nodes: {nodes}")
        )],
        "{message}"
      )
    })
  }

  fn get_hint(&self, parent: &KdlNode, nodes: &KdlDocument) -> Result<String, ConfigError> {
    let hint = nodes.get("hint").ok_or_else(|| {
      diagnostic!(
        source = &self.source,
        code = "decaff::config::actions",
        labels = vec![LabeledSpan::at(
          parent.span().to_owned(),
          "prompts require a `hint` child node"
        )],
        "Missing prompt hint."
      )
    })?;

    self.get_arg_string(hint)
  }

  fn get_injects(&self, node: &KdlNode) -> Option<HashSet<String>> {
    node.children().map(|children| {
      children
        .get_args("inject")
        .into_iter()
        .filter_map(|arg| arg.as_string().map(str::to_string))
        .collect()
    })
  }

  fn get_options(&self, parent: &KdlNode, nodes: &KdlDocument) -> Result<Vec<String>, ConfigError> {
    let options = nodes.get("options").ok_or_else(|| {
      diagnostic!(
        source = &self.source,
        code = "decaff::config::actions",
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
          code = "decaff::config::actions",
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
          code = "decaff::config::actions",
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

  fn get_default_number(&self, nodes: &KdlDocument) -> Option<Number> {
    nodes.get("default").and_then(|node| node.get_number(0))
  }
}
