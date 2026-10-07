//! `+` and `-` as BDS's optimiser builds them: one flat sum, its literal moved into the sum's own
//! post-op, and its terms merged (`optimise.rs` has the rest of the model).

use crate::parse::Parser;
use crate::post::Post;
use crate::program::Node;

/// A term of a sum: `base` through `post`, and the node that already says so, until `post` changes.
#[derive(Clone, Copy)]
pub(crate) struct Summand {
    pub(crate) base: u32,
    pub(crate) post: Post,
    node: Option<u32>,
}

impl Parser<'_> {
    /// `left + right`; for `left - right`, `right` is negated first.
    pub(crate) fn sum(&mut self, left: u32, right: u32) -> u32 {
        if let (Some(x), Some(y)) = (self.raw(left), self.raw(right)) {
            return self.number(x + y);
        }
        // The sum's own post-op is (1, offset).
        let mut offset = 0.0f32;
        let mut terms: Vec<Summand> = Vec::new();
        for child in [left, right] {
            let (of, post) = self.split(child);
            let Node::Sum(list) = self.node(of) else {
                terms.push(Summand { base: of, post, node: Some(child) });
                continue;
            };
            for &term in self.items(list) {
                let (base, own) = self.split(term);
                terms.push(if post.scale == 1.0 {
                    Summand { base, post: own, node: Some(term) }
                } else {
                    Summand { base, post: Post { scale: own.scale * post.scale, offset: own.offset * post.scale }, node: None }
                });
            }
            offset += post.offset;
        }
        let mut constant = None;
        terms.retain(|term| match self.raw(term.base) {
            Some(n) => {
                constant = Some(n * term.post.scale + term.post.offset);
                false
            }
            None => true,
        });
        if let Some(c) = constant {
            if let [term] = terms[..] {
                return self.wrap((term.base, Post { scale: term.post.scale, offset: (term.post.offset + c) + offset }));
            }
            offset += c;
        } else if self.terms_alike(&terms) {
            return self.merge_alike(&terms, offset);
        }
        self.merge_variables(&mut terms);
        match terms[..] {
            [] => self.number_with(offset),
            [term] => self.wrap((term.base, Post { scale: term.post.scale, offset: term.post.offset + offset })),
            _ => {
                let terms: Vec<u32> = terms.into_iter().map(|term| term.node.unwrap_or_else(|| self.wrap((term.base, term.post)))).collect();
                let list = self.list(&terms);
                let sum = self.push(Node::Sum(list));
                self.wrap((sum, Post { scale: 1.0, offset }))
            }
        }
    }

    /// Terms that print alike become one, their post-ops summed in two interleaved chains (even and
    /// odd positions), then the odd one out.
    fn merge_alike(&mut self, terms: &[Summand], offset: f32) -> u32 {
        let zero = Post { scale: 0.0, offset: 0.0 };
        let (mut even, mut odd) = (zero, zero);
        let (pairs, remainder) = terms.as_chunks::<2>();
        for [a, b] in pairs {
            even = a.post.plus(even);
            odd = b.post.plus(odd);
        }
        let mut total = odd.plus(even);
        if let [last] = remainder {
            total = last.post.plus(total);
        }
        if total.scale == 0.0 {
            return self.number_with(offset);
        }
        self.wrap((terms[0].base, Post { scale: total.scale, offset: total.offset + offset }))
    }

    /// Terms reading the same plain `variable.` merge into the first, their post-ops summed; when the
    /// scale cancels to 0 the merged term goes, unread, and a later one starts afresh.
    fn merge_variables(&self, terms: &mut Vec<Summand>) {
        let mut live: Vec<(u32, usize)> = Vec::new();
        let mut leaving = vec![false; terms.len()];
        for index in 0..terms.len() {
            let Node::Var { slot, path } = self.node(terms[index].base) else { continue };
            if path.len > 0 {
                continue;
            }
            let Some(at) = live.iter().position(|&(held, _)| held == slot) else {
                live.push((slot, index));
                continue;
            };
            let first = live[at].1;
            leaving[index] = true;
            terms[first].post = terms[first].post.plus(terms[index].post);
            terms[first].node = None;
            if terms[first].post.scale == 0.0 {
                leaving[first] = true;
                live.remove(at);
            }
        }
        let mut index = 0;
        terms.retain(|_| {
            index += 1;
            !leaving[index - 1]
        });
    }
}
