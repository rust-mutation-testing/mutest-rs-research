use std::path::{Component, PathBuf};

fn get_path_normals(path: &PathBuf) -> Vec<String> {
    path.components().filter_map(|c| {
        if let Component::Normal(c) = c {
            Some(c.display().to_string())
        } else {
            None
        }
    }).collect()
}

pub struct Node {
    value: String,
    children: Vec<Node>,
}

impl Node {
    pub fn new(value: String) -> Self {
        Node { value, children: Vec::new() }
    }

    pub fn default() -> Self {
        Node { value: "".to_owned(), children: Vec::new() }
    }

    pub fn insert(&mut self, child: Node) {
        for c in &self.children {
            if c.value == child.value {
                return;
            }
        }
        self.children.push(child);
    }

    pub fn find_node(&mut self, path: &PathBuf) -> Option<&mut Node> {
        let normals = get_path_normals(path);
        if normals.is_empty() {
            return None;
        }
        for child in &mut self.children {
            if child.value == normals[0] {
                if normals.len() == 1 {
                    return Some(child);
                }
                return child.find_node(&PathBuf::from_iter(&normals[1..]))
            }
        }
        None
    }

    pub fn sort(&mut self) {
        self.children.sort_by(|a, b| {
            let a_is_folder = !a.value.contains('.');
            let b_is_folder = !b.value.contains('.');

            if a_is_folder != b_is_folder {
                return b_is_folder.cmp(&a_is_folder);
            }

            a.value.cmp(&b.value)
        });
        for child in &mut self.children {
            child.sort();
        }
    }

    pub fn is_folder(&self) -> bool {
        self.children.len() != 0
    }
}

pub struct FileTree {
    pub root_node: Node,
}

impl FileTree {
    pub fn new() -> Self {
        FileTree { root_node: Node::default() }
    }

    pub fn insert_path(&mut self, path: &PathBuf) {
        let mut prefix = PathBuf::new();
        for normal in get_path_normals(&path) {
            self.insert_node(&prefix, Node { value: normal.clone(), children: vec![] });
            prefix = prefix.join(normal);
        }
    }

    fn insert_node(&mut self, prefix: &PathBuf, node: Node) {
        if let Some(n) = self.root_node.find_node(prefix) {
            n.insert(node);
        } else {
            self.root_node.insert(node);
        }
    }

    pub fn sort(&mut self) {
        self.root_node.sort();
    }
}