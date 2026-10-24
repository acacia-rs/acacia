//! Bots mowing their band of the world: passes along x, `stride` chunks apart in z, a square view of
//! `radius` around them. A step yields the chunks that entered the view, as a server sends them.
pub type Pos = (i32, i32);

pub struct Bot {
    pub at: Pos,
    dir: i32,
    band_end: i32,
    pub done: bool,
    /// Steps left of the turn onto the next pass.
    turn: i32,
    /// Fractional chunks of movement owed.
    carry: f64,
}

pub struct Field {
    pub side: i32,
    pub radius: i32,
    pub stride: i32,
}

impl Field {
    fn inside(&self, (x, z): Pos) -> bool {
        (0..self.side).contains(&x) && (0..self.side).contains(&z)
    }

    /// Every chunk the bot at `at` sees.
    pub fn view(&self, at: Pos, out: &mut Vec<Pos>) {
        let r = self.radius;
        for z in at.1 - r..=at.1 + r {
            for x in at.0 - r..=at.0 + r {
                if self.inside((x, z)) {
                    out.push((x, z));
                }
            }
        }
    }

    /// `n` bots over equal bands of rows, each starting at x 0 on its band's first pass.
    pub fn bots(&self, n: i32) -> Vec<Bot> {
        (0..n)
            .map(|i| {
                let (z0, z1) = (self.side * i / n, self.side * (i + 1) / n);
                Bot {
                    at: (0, (z0 + self.radius).min(z1 - 1)),
                    dir: 1,
                    band_end: z1,
                    done: false,
                    turn: 0,
                    carry: 0.0,
                }
            })
            .collect()
    }

    /// Moves `bot` by `chunks` (fractions carried over), pushing the chunks that came into view.
    pub fn advance(&self, bot: &mut Bot, chunks: f64, out: &mut Vec<Pos>) {
        bot.carry += chunks;
        while bot.carry >= 1.0 && !bot.done {
            bot.carry -= 1.0;
            let from = bot.at;
            let x = from.0 + bot.dir;
            if bot.turn > 0 {
                bot.at.1 += 1;
                bot.turn -= 1;
                if bot.turn == 0 {
                    bot.dir = -bot.dir;
                }
            } else if (0..self.side).contains(&x) {
                bot.at.0 = x;
            } else if from.1 - self.radius + self.stride >= bot.band_end {
                bot.done = true;
                return;
            } else {
                bot.at.1 += 1;
                bot.turn = self.stride - 1;
                if bot.turn == 0 {
                    bot.dir = -bot.dir;
                }
            }
            let r = self.radius;
            let start = out.len();
            self.view(bot.at, out);
            let mut i = start;
            while i < out.len() {
                let p = out[i];
                if (p.0 - from.0).abs() <= r && (p.1 - from.1).abs() <= r {
                    out.swap_remove(i);
                } else {
                    i += 1;
                }
            }
        }
    }
}
