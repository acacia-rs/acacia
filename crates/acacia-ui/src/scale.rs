//! GUI scale: window pixels per GUI pixel.

/// The largest whole scale that leaves at least 320×240 GUI pixels (Java's `Window.calculateScale`),
/// capped by `chosen` unless it is 0 (auto).
pub fn gui_scale(width: u32, height: u32, chosen: u32) -> u32 {
    let mut scale = 1;
    while (chosen == 0 || scale < chosen) && width / (scale + 1) >= 320 && height / (scale + 1) >= 240 {
        scale += 1;
    }
    scale
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auto_takes_the_largest_that_fits() {
        assert_eq!(gui_scale(854, 480, 0), 2);
        assert_eq!(gui_scale(1920, 1080, 0), 4);
        assert_eq!(gui_scale(1920, 1080, 2), 2);
        assert_eq!(gui_scale(300, 200, 0), 1, "never below 1");
    }
}
