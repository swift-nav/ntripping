//! makes the git tag info available at build time
use vergen_gitcl::{Emitter, Gitcl};

fn main() {
    // Make vergen `git describe` to populate some env vars at build time
    let gitcl = Gitcl::builder()
        .describe(
            true, // --dirty
            true, // --tags [to allow lightweight tags]
            None, // no glob matches passed in
        )
        .build();

    Emitter::default()
        .add_instructions(&gitcl)
        .expect("Unable to add git instructions")
        .emit()
        .expect("Unable to emit instructions");
}
