//! `CommandDispatcher.getSmartUsage`: each child's usage, optional parts in `[...]`, alternatives
//! in `(a|b)`.

use super::tree::Tree;

impl Tree {
    /// `getSmartUsage(node, source)`: per child, in order.
    pub fn smart_usage(&self, node: usize) -> Vec<(usize, String)> {
        let optional = self.nodes[node].executable;
        self.nodes[node].children.iter().map(|&c| (c, self.child_usage(c, optional, false))).collect()
    }

    fn child_usage(&self, node: usize, optional: bool, deep: bool) -> String {
        let n = &self.nodes[node];
        let own = if optional { format!("[{}]", n.usage()) } else { n.usage() };
        if deep {
            return own;
        }
        let child_optional = n.executable;
        let (open, close) = if child_optional { ("[", "]") } else { ("(", ")") };
        match n.children.as_slice() {
            [] => own,
            [only] => format!("{own} {}", self.child_usage(*only, child_optional, child_optional)),
            children => {
                let mut usages: Vec<String> = Vec::new();
                for &c in children {
                    let u = self.child_usage(c, child_optional, true);
                    if !usages.contains(&u) {
                        usages.push(u);
                    }
                }
                if let [only] = usages.as_slice() {
                    return format!("{own} {}", if child_optional { format!("[{only}]") } else { only.clone() });
                }
                let names: Vec<String> = children.iter().map(|&c| self.nodes[c].usage()).collect();
                format!("{own} {open}{}{close}", names.join("|"))
            }
        }
    }
}
