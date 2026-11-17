/// Elytra wear as BDS `Mob::updateGlidingDurability` (1429e8df0) takes it: one use on each glide tick whose
/// count is 19 mod 20. The server sends no damage until the elytra is spent, so the client counts.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct GlideWear {
    ticks: u32,
    used: u32,
    /// The `Equipment::elytra_uses` counted against; a new value restarts the count.
    of: Option<u32>,
}

impl GlideWear {
    pub(crate) fn sync(&mut self, uses: Option<u32>, gliding: bool) {
        if self.of != uses {
            *self = Self { of: uses, ..Self::default() };
        }
        if !gliding {
            self.ticks = 0;
        }
    }

    pub(crate) fn usable(&self, uses: Option<u32>) -> bool {
        uses.is_none_or(|n| self.used < n)
    }

    pub(crate) fn step(&mut self) {
        self.ticks += 1;
        if self.ticks % 20 == 19 {
            self.used += 1;
        }
    }
}
