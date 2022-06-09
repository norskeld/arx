use petgraph::stable_graph::StableDiGraph;
use petgraph::Direction;

pub trait Node {
  type Item;

  fn dependencies(&self) -> &[Self::Item];
  fn matches(&self, dep: &Self::Item) -> bool;
}

#[derive(Debug)]
pub enum Step<'a, N: Node> {
  Resolved(&'a N),
  Unresolved(&'a N::Item),
}

impl<'a, N: Node> Step<'a, N> {
  pub fn is_resolved(&self) -> bool {
    match self {
      | Step::Resolved(_) => true,
      | Step::Unresolved(_) => false,
    }
  }

  pub fn as_resolved(&self) -> Option<&N> {
    match self {
      | Step::Resolved(node) => Some(node),
      | Step::Unresolved(_) => None,
    }
  }

  pub fn as_unresolved(&self) -> Option<&N::Item> {
    match self {
      | Step::Resolved(_) => None,
      | Step::Unresolved(requirement) => Some(requirement),
    }
  }
}

#[derive(Debug)]
pub struct DependenciesGraph<'a, N: Node> {
  graph: StableDiGraph<Step<'a, N>, &'a N::Item>,
}

impl<'a, N> From<&'a [N]> for DependenciesGraph<'a, N>
where
  N: Node,
{
  fn from(nodes: &'a [N]) -> Self {
    let mut graph = StableDiGraph::<Step<'a, N>, &'a N::Item>::new();

    // Insert the input nodes into the graph, and record their positions. We'll be adding the edges
    // next, and filling in any unresolved steps we find along the way.
    let nodes: Vec<(_, _)> = nodes
      .iter()
      .map(|node| (node, graph.add_node(Step::Resolved(node))))
      .collect();

    for (node, index) in nodes.iter() {
      for dependency in node.dependencies() {
        // Check to see if we can resolve this dependency internally.
        if let Some((_, dependent)) = nodes.iter().find(|(dep, _)| dep.matches(dependency)) {
          // If we can, just add an edge between the two nodes.
          graph.add_edge(*index, *dependent, dependency);
        } else {
          // If not, create a new Unresolved node, and create an edge to that.
          let unresolved = graph.add_node(Step::Unresolved(dependency));
          graph.add_edge(*index, unresolved, dependency);
        }
      }
    }

    Self { graph }
  }
}

impl<'a, N> DependenciesGraph<'a, N>
where
  N: Node,
{
  pub fn is_resolvable(&self) -> bool {
    self.graph.node_weights().all(Step::is_resolved)
  }

  pub fn unresolved(&self) -> impl Iterator<Item = &N::Item> {
    self.graph.node_weights().filter_map(Step::as_unresolved)
  }
}

impl<'a, N> Iterator for DependenciesGraph<'a, N>
where
  N: Node,
{
  type Item = Step<'a, N>;

  fn next(&mut self) -> Option<Self::Item> {
    // Returns the first node, which does not have any Outgoing edges, which means it is terminal.
    for index in self.graph.node_indices().rev() {
      let neighbors = self.graph.neighbors_directed(index, Direction::Outgoing);

      if neighbors.count() == 0 {
        return self.graph.remove_node(index);
      }
    }

    None
  }
}
