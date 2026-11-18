//! Titles, the action bar, system chat, the tab list's header and footer, and the vitals the HUD shows.

use crate::VarInt;
use crate::text::Component;

body!(SetTitleText { text: Component });
body!(SetSubtitleText { text: Component });
body!(SetActionBarText { text: Component });
// Fade in, stay and fade out in ticks, as three plain ints.
body!(SetTitlesAnimation { fade_in: i32, stay: i32, fade_out: i32 });
body!(ClearTitles { reset_times: bool });
// `overlay` puts the line in the action bar instead of the chat.
body!(SystemChat { content: Component, overlay: bool });
body!(TabList { header: Component, footer: Component });
// Wire order is progress, level, total; the Java constructor takes total before level.
body!(SetExperience { progress: f32, level: VarInt, total: VarInt });
