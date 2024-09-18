use tree_sitter::{Node, TreeCursor};
/// Utilities to simplify parsing tree-sitter structures.

pub struct TreeWalkerDepthFirst<'a> {
    cursor: TreeCursor<'a>,
}

impl<'a> Iterator for TreeWalkerDepthFirst<'a> {
    type Item = Node<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.cursor.goto_first_child() {
            return Some(self.cursor.node());
        }
        if self.cursor.goto_next_sibling() {
            return Some(self.cursor.node());
        }
        while self.cursor.goto_parent() {
            if self.cursor.goto_next_sibling() {
                return Some(self.cursor.node());
            }
        }
        None
    }
}

pub fn nodes_depth_first<'a>(node: &'a Node) -> impl Iterator<Item = Node<'a>> {
    TreeWalkerDepthFirst {
        cursor: node.walk(),
    }
}

pub fn named_nodes_depth_first<'a>(node: &'a Node) -> impl Iterator<Item = Node<'a>> {
    nodes_depth_first(node).filter(|&x| x.is_named())
}

/// Get the first child with a given name. Returns None if not found.
pub fn child_with_name<'a>(node: &'a Node, name: &'a str) -> Option<Node<'a>> {
    node.named_children(&mut node.walk())
        .find(|x| x.kind() == name)
}

// Convert a node to text, collapsing any raised errors to None.
pub fn to_text<'a>(node: &'a Node<'a>, src: &'a str) -> Option<&'a str> {
    node.utf8_text(src.as_bytes()).ok()
}

/// Strip line breaks from a string of Fortran code.
pub fn strip_line_breaks(src: &str) -> String {
    src.replace("&", "").replace("\n", " ")
}

/// Given a variable declaration or function statement, return its type if it's an intrinsic type,
/// or None otherwise.
pub fn intrinsic_type(node: &Node) -> Option<String> {
    if let Some(child) = child_with_name(node, "intrinsic_type") {
        let grandchild = child.child(0)?;
        return Some(grandchild.kind().to_string());
    }
    None
}

/// Returns true if the type passed to it is number-like.
/// Deliberately does not include 'double precision' or 'double complex'.
pub fn dtype_is_number(dtype: &str) -> bool {
    matches!(
        dtype.to_lowercase().as_str(),
        "integer" | "real" | "logical" | "complex"
    )
}
