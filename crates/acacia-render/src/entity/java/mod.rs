//! Java's entity models drawn as Java draws them: the baked layer trees the game builds in code
//! (exported from the user's own client jar, never shipped), `setupAnim` ported per model
//! family, and `ModelPart.render`'s walk. See README "Java entity models".

pub mod anim;
pub mod avatar;
pub mod bedrock;
pub mod draw;
pub mod equipment;
pub mod joml;
pub mod kinds;
pub mod library;
pub mod mesh;
pub mod model;
pub mod mth;
pub mod overlay;
pub mod poses;
pub mod state;
pub mod trims;
pub mod types;

pub use anim::Family;
pub use model::{Part, PartPose, Polygon};
pub use poses::Poses;
pub use state::{Field, RenderState};
