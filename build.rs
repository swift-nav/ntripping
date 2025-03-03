//! makes the git tag info available at build time
use vergen_gitcl::{Emitter, GitclBuilder};

fn main() {
    // Make vergen `git describe` to populate some env vars at build time
    let gitcl = GitclBuilder::default()
        .describe(
            true, // --dirty
            true, // --tags [to allow lightweight tags]
            None, // no glob matches passed in
        )
        .build()
        .expect("Unable to build. Please ensure git is installed");

    Emitter::default()
        .add_instructions(&gitcl)
        .expect("Unable to add git instructions")
        .emit()
        .expect("Unable to emit instructions");
}
