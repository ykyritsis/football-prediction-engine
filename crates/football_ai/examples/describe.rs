//! Print generated descriptions for the team archetypes and example players.
//!
//! ```text
//! cargo run -p football_ai --example describe
//! ```

use football_ai::identity::presets::{self, example_messi, example_ronaldo, Squad};

fn main() {
    for p in [example_messi(), example_ronaldo()] {
        println!("{}\n", p.describe());
    }
    for style in presets::team_archetypes() {
        println!("{}\n", Squad::generated(style, 80).describe());
    }
}
