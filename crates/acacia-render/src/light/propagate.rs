//! Flood-fill light propagation over [`LightData`], one channel at a time. A step loses
//! 1 + the entered block's filter; sky light at 15 going down loses only the filter.
//! Removal follows the usual two-queue scheme: clear what may have come from the removed light,
//! then refill from every brighter cell found on the way.

use std::collections::VecDeque;

use super::data::LightData;

const DIRS: [[i32; 3]; 6] = [[1, 0, 0], [-1, 0, 0], [0, 1, 0], [0, -1, 0], [0, 0, 1], [0, 0, -1]];
const DOWN: usize = 3;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Channel {
    Block,
    Sky,
}

impl Channel {
    pub const fn shift(self) -> u32 {
        match self {
            Channel::Block => 4,
            Channel::Sky => 0,
        }
    }
}

type Pos = [i32; 3];

pub struct Propagator<'a> {
    pub data: &'a mut LightData,
    pub channel: Channel,
    /// First y above the world: sky light enters the top layer from there.
    pub top: i32,
    increase: VecDeque<Pos>,
    decrease: VecDeque<(Pos, u8)>,
}

impl<'a> Propagator<'a> {
    pub fn new(data: &'a mut LightData, channel: Channel, top: i32) -> Self {
        Propagator { data, channel, top, increase: VecDeque::new(), decrease: VecDeque::new() }
    }

    #[inline]
    pub fn level(&self, [x, y, z]: Pos) -> Option<u8> {
        self.data.light(x, y, z).map(|l| (l >> self.channel.shift()) & 15)
    }

    #[inline]
    pub fn set(&mut self, [x, y, z]: Pos, level: u8) {
        let Some(old) = self.data.light(x, y, z) else { return };
        let shift = self.channel.shift();
        let value = (old & !(15 << shift)) | (level << shift);
        if value != old {
            self.data.set_light(x, y, z, value);
        }
    }

    #[inline]
    fn filter(&self, [x, y, z]: Pos) -> u8 {
        self.data.props.get(x, y, z).map_or(15, |p| p & 15)
    }

    /// Light the block makes by itself: emission, or sky light entering the top layer.
    pub fn source(&self, p: Pos) -> u8 {
        match self.channel {
            Channel::Block => self.data.props.get(p[0], p[1], p[2]).map_or(0, |v| v >> 4),
            Channel::Sky if p[1] == self.top - 1 => step(15, self.filter(p), true),
            Channel::Sky => 0,
        }
    }

    /// Queues a block whose level is final, to spread from.
    pub fn spread_from(&mut self, p: Pos) {
        self.increase.push_back(p);
    }

    /// Sets a block's level and spreads from it, when brighter than what it has.
    pub fn raise(&mut self, p: Pos, level: u8) {
        if self.level(p).is_some_and(|l| level > l) {
            self.set(p, level);
            self.increase.push_back(p);
        }
    }

    /// Clears a block's light and everything that may have come from it; [`Propagator::run`]
    /// then refills from the remaining light.
    pub fn remove(&mut self, p: Pos) {
        let Some(level) = self.level(p) else { return };
        if level > 0 {
            self.set(p, 0);
            self.decrease.push_back((p, level));
        }
        let s = self.source(p);
        self.raise(p, s);
        for d in DIRS {
            let n = [p[0] + d[0], p[1] + d[1], p[2] + d[2]];
            if self.level(n).is_some_and(|l| l > 0) {
                self.increase.push_back(n);
            }
        }
    }

    pub fn run(&mut self) {
        while let Some((p, level)) = self.decrease.pop_front() {
            for (i, d) in DIRS.iter().enumerate() {
                let n = [p[0] + d[0], p[1] + d[1], p[2] + d[2]];
                let Some(nl) = self.level(n) else { continue };
                if nl == 0 {
                    continue;
                }
                let from_us = nl < level || (self.channel == Channel::Sky && i == DOWN && level == 15);
                if from_us {
                    self.set(n, 0);
                    self.decrease.push_back((n, nl));
                    let s = self.source(n);
                    self.raise(n, s);
                } else {
                    self.increase.push_back(n);
                }
            }
        }
        while let Some(p) = self.increase.pop_front() {
            let Some(level) = self.level(p) else { continue };
            if level <= 1 {
                continue;
            }
            for (i, d) in DIRS.iter().enumerate() {
                let n = [p[0] + d[0], p[1] + d[1], p[2] + d[2]];
                let Some(nl) = self.level(n) else { continue };
                let to = step(level, self.filter(n), self.channel == Channel::Sky && i == DOWN);
                if to > nl {
                    self.set(n, to);
                    self.increase.push_back(n);
                }
            }
        }
    }
}

/// Level after entering a block with `filter`.
#[inline]
pub fn step(level: u8, filter: u8, sky_down: bool) -> u8 {
    if sky_down && level == 15 { 15u8.saturating_sub(filter) } else { level.saturating_sub(1 + filter) }
}
