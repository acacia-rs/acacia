//! `array.name[index]`: a render controller's arrays, compiled into the program that indexes them.

use crate::error::{Error, ErrorKind};
use crate::parse::Parser;
use crate::program::Node;

impl Parser<'_> {
    pub(crate) fn index(&mut self, array: &str) -> Result<u32, Error> {
        let unknown = self.fail_last(ErrorKind::UnknownArray(array.to_owned()));
        self.expect("[")?;
        let index = self.expr()?;
        self.expect("]")?;
        let mut items = Vec::new();
        self.elements(array, &unknown, 0, &mut items)?;
        if items.is_empty() {
            return Err(unknown);
        }
        let items = self.list(&items);
        Ok(self.push(Node::Index { items, index }))
    }

    /// Compiles an array's elements into `out`. An element that names an array stands for all of
    /// that array's elements.
    fn elements(&mut self, array: &str, unknown: &Error, depth: u8, out: &mut Vec<u32>) -> Result<(), Error> {
        let sources = self.arrays.and_then(|arrays| arrays.get(array)).filter(|_| depth < 8).ok_or_else(|| unknown.clone())?;
        for source in sources {
            let lower = source.trim().to_ascii_lowercase();
            match lower.strip_prefix("array.").filter(|name| !name.contains('[')) {
                Some(included) => self.elements(included, unknown, depth + 1, out)?,
                None => out.push(self.inline(source)?),
            }
        }
        Ok(())
    }
}
