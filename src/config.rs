use kdl::{KdlDocument, KdlEntry, KdlNode};

use crate::app::AppError;
use crate::graph::{DependenciesGraph, Node, Step};

#[derive(Debug)]
pub struct Replacement {
  pub tag: String,
  pub description: Option<String>,
}

#[derive(Debug)]
pub enum Action {
  Suite(Vec<ActionSuite>),
  Single(Vec<ActionSingle>),
}

#[derive(Clone, Debug)]
pub struct ActionSuite {
  pub name: String,
  pub actions: Vec<ActionSingle>,
  pub requirements: Vec<String>,
}

impl Node for ActionSuite {
  type Item = String;

  fn dependencies(&self) -> &[Self::Item] {
    &self.requirements[..]
  }

  fn matches(&self, dependency: &Self::Item) -> bool {
    self.name == *dependency
  }
}

#[derive(Clone, Debug)]
pub enum ActionSingle {
  Copy {
    from: Option<String>,
    to: Option<String>,
    overwrite: bool,
  },
  Move {
    from: Option<String>,
    to: Option<String>,
    overwrite: bool,
  },
  Delete {
    target: Option<String>,
  },
  Run {
    command: Option<String>,
  },
  Unknown,
}

pub fn resolve_requirements(suites: &[ActionSuite]) -> (Vec<ActionSuite>, Vec<String>) {
  let graph = DependenciesGraph::from(suites);

  graph.fold((vec![], vec![]), |(mut resolved, mut unresolved), next| {
    match next {
      | Step::Resolved(suite) => resolved.push(suite.clone()),
      | Step::Unresolved(dep) => unresolved.push(dep.clone()),
    }

    (resolved, unresolved)
  })
}

pub fn get_actions(doc: &KdlDocument) -> Option<Action> {
  doc
    .get("actions")
    .and_then(|node| node.children())
    .map(|children| {
      let nodes = children.nodes();

      if nodes.iter().all(is_suite) {
        let suites = nodes.iter().filter_map(to_action_suite).collect::<Vec<_>>();
        Action::Suite(suites)
      } else {
        let actions = nodes.iter().filter_map(to_action).collect::<Vec<_>>();
        Action::Single(actions)
      }
    })
}

pub fn get_replacements(doc: &KdlDocument) -> Option<Vec<Replacement>> {
  doc
    .get("replacements")
    .and_then(|node| node.children())
    .map(|children| {
      children
        .nodes()
        .iter()
        .filter_map(to_replacement)
        .collect::<Vec<_>>()
    })
}

fn to_replacement(node: &KdlNode) -> Option<Replacement> {
  let tag = node.name().to_string();
  let description = node.get(0).and_then(entry_to_string);

  Some(Replacement { tag, description })
}

fn to_action(node: &KdlNode) -> Option<ActionSingle> {
  let action = to_action_single(node);

  if let ActionSingle::Unknown = action {
    None
  } else {
    Some(action)
  }
}

fn to_action_suite(node: &KdlNode) -> Option<ActionSuite> {
  let name = node.get("name").and_then(entry_to_string);
  let requirements = node.get("requires").and_then(entry_to_string).map(|value| {
    value
      .split_ascii_whitespace()
      .map(str::to_string)
      .collect::<Vec<_>>()
  });

  let actions = node.children().map(|children| {
    children
      .nodes()
      .iter()
      .map(to_action_single)
      .collect::<Vec<_>>()
  });

  let suite = (
    name,
    actions.unwrap_or(vec![]),
    requirements.unwrap_or(vec![]),
  );

  match suite {
    | (Some(name), actions, requirements) => Some(ActionSuite {
      name,
      actions,
      requirements,
    }),
    | _ => None,
  }
}

fn to_action_single(node: &KdlNode) -> ActionSingle {
  let action_kind = node.name().to_string();

  match action_kind.to_ascii_lowercase().as_str() {
    | "copy" => ActionSingle::Copy {
      from: node.get("from").and_then(entry_to_string),
      to: node.get("to").and_then(entry_to_string),
      overwrite: node
        .get("overwrite")
        .and_then(|value| value.value().as_bool())
        .unwrap_or(true),
    },
    | "move" => ActionSingle::Move {
      from: node.get("from").and_then(entry_to_string),
      to: node.get("to").and_then(entry_to_string),
      overwrite: node
        .get("overwrite")
        .and_then(|value| value.value().as_bool())
        .unwrap_or(true),
    },
    | "delete" => ActionSingle::Delete {
      target: node.get(0).and_then(entry_to_string),
    },
    | "run" => ActionSingle::Run {
      command: node.get(0).and_then(entry_to_string),
    },
    | _ => ActionSingle::Unknown,
  }
}

fn is_suite(node: &KdlNode) -> bool {
  node.name().value().to_string().eq("suite".into())
}

fn entry_to_string(entry: &KdlEntry) -> Option<String> {
  // dbg!(entry.value());
  entry.value().as_string().map(str::to_string)
}
