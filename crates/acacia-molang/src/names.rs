use std::collections::HashMap;

/// Names numbered in the order they were first seen.
#[derive(Debug, Default)]
pub(crate) struct Names {
    ids: HashMap<Box<str>, u32>,
    names: Vec<Box<str>>,
}

impl Names {
    pub(crate) fn intern(&mut self, name: &str) -> u32 {
        if let Some(&id) = self.ids.get(name) {
            return id;
        }
        let id = self.names.len() as u32;
        self.names.push(name.into());
        self.ids.insert(name.into(), id);
        id
    }

    pub(crate) fn find(&self, name: &str) -> Option<u32> {
        self.ids.get(name).copied()
    }

    pub(crate) fn name(&self, id: u32) -> &str {
        &self.names[id as usize]
    }

    pub(crate) fn len(&self) -> usize {
        self.names.len()
    }
}
