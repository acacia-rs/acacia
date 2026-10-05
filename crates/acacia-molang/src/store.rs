//! Variable slots and the structs they hold. Structs are values: assigning one copies it, so every
//! struct here has exactly one owner (a slot or a member of another struct).

use crate::value::{StructRef, Symbol, Value};

#[derive(Debug, Default)]
pub(crate) struct Store {
    slots: Vec<Option<Value>>,
    structs: Vec<Vec<(Symbol, Value)>>,
    free: Vec<u32>,
    /// What this store's [`StructRef`]s carry, so a value says which store it lives in.
    scratch: bool,
}

impl Store {
    pub(crate) fn scratch() -> Store {
        Store { scratch: true, ..Store::default() }
    }

    /// Empties it, keeping every allocation.
    pub(crate) fn reset(&mut self, slots: usize) {
        self.slots.clear();
        self.slots.resize(slots, None);
        self.free.clear();
        for (index, members) in self.structs.iter_mut().enumerate() {
            members.clear();
            self.free.push(index as u32);
        }
    }

    pub(crate) fn owns(&self, value: Value) -> bool {
        value.storage().is_some_and(|(r, _)| r.scratch == self.scratch)
    }

    pub(crate) fn get(&self, slot: u32, path: &[u32]) -> Option<Value> {
        let mut value = (*self.slots.get(slot as usize)?)?;
        for &member in path {
            value = self.member(value, Symbol(member))?;
        }
        Some(value)
    }

    pub(crate) fn member(&self, of: Value, name: Symbol) -> Option<Value> {
        let Value::Struct(r) = of else { return None };
        self.structs[r.index as usize].iter().find(|(n, _)| *n == name).map(|(_, v)| *v)
    }

    pub(crate) fn members(&self, of: StructRef) -> &[(Symbol, Value)] {
        &self.structs[of.index as usize]
    }

    /// `value` must not be a struct of another store: [`Store::import`] it first.
    pub(crate) fn set(&mut self, slot: u32, path: &[u32], value: Value) {
        if self.slots.len() <= slot as usize {
            self.slots.resize(slot as usize + 1, None);
        }
        let Some((&last, parents)) = path.split_last() else {
            let old = self.slots[slot as usize].replace(value);
            return self.release(old);
        };
        let mut holder = match self.slots[slot as usize] {
            Some(Value::Struct(r)) => r,
            old => {
                let made = self.new_struct();
                self.slots[slot as usize] = Some(Value::Struct(made));
                self.release(old);
                made
            }
        };
        for &member in parents {
            holder = match self.member(Value::Struct(holder), Symbol(member)) {
                Some(Value::Struct(r)) => r,
                _ => {
                    let made = self.new_struct();
                    let old = self.set_member(holder, Symbol(member), Value::Struct(made));
                    self.release(old);
                    made
                }
            };
        }
        let old = self.set_member(holder, Symbol(last), value);
        self.release(old);
    }

    /// [`Store::set`] for a path of at most 8 members given as symbols.
    pub(crate) fn set_symbols(&mut self, slot: u32, path: &[Symbol], value: Value) {
        let mut raw = [0; 8];
        assert!(path.len() <= raw.len(), "member path deeper than 8");
        for (id, symbol) in raw.iter_mut().zip(path) {
            *id = symbol.0;
        }
        self.set(slot, &raw[..path.len()], value);
    }

    pub(crate) fn new_struct(&mut self) -> StructRef {
        let index = self.free.pop().unwrap_or_else(|| {
            self.structs.push(Vec::new());
            self.structs.len() as u32 - 1
        });
        StructRef { index, scratch: self.scratch }
    }

    pub(crate) fn set_member(&mut self, of: StructRef, name: Symbol, value: Value) -> Option<Value> {
        let members = &mut self.structs[of.index as usize];
        match members.iter_mut().find(|(n, _)| *n == name) {
            Some((_, slot)) => Some(std::mem::replace(slot, value)),
            None => {
                members.push((name, value));
                None
            }
        }
    }

    /// A copy of `value` owned by this store; `from` is the store a struct lives in, `None` for this one.
    pub(crate) fn import(&mut self, value: Value, from: Option<&Store>) -> Value {
        let Some((source, kind)) = value.storage() else { return value };
        let made = self.new_struct();
        let count = from.unwrap_or(self).members(source).len();
        for i in 0..count {
            let (name, member) = from.unwrap_or(self).members(source)[i];
            let member = self.import(member, from);
            self.structs[made.index as usize].push((name, member));
        }
        kind(made)
    }

    /// Appends without looking for the name: for array elements, whose names are their positions.
    pub(crate) fn push_member(&mut self, of: StructRef, value: Value) {
        let members = &mut self.structs[of.index as usize];
        members.push((Symbol(members.len() as u32), value));
    }

    fn release(&mut self, value: Option<Value>) {
        let Some((r, _)) = value.and_then(Value::storage) else { return };
        if r.scratch != self.scratch {
            return;
        }
        while let Some((_, member)) = self.structs[r.index as usize].pop() {
            self.release(Some(member));
        }
        self.free.push(r.index);
    }
}
