//! World border packets.

#![allow(unused_doc_comments)] // docs on `packet!` fields' structs

use super::VarLong;
use crate::packets::packet;
use crate::{Error, VarInt, Wire};

/// All sizes are in blocks; `lerp_time` in milliseconds.
packet!(InitializeBorder {
    center_x: f64,
    center_z: f64,
    size: f64,
    lerp_target: f64,
    lerp_time: VarLong,
    absolute_max_size: VarInt,
    warning_blocks: VarInt,
    warning_time: VarInt,
});
packet!(SetBorderCenter { x: f64, z: f64 });
packet!(SetBorderLerpSize { old_size: f64, new_size: f64, lerp_time: VarLong });
packet!(SetBorderSize { size: f64 });
packet!(SetBorderWarningDelay { seconds: VarInt });
packet!(SetBorderWarningDistance { blocks: VarInt });
