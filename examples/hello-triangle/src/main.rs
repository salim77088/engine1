//! Smallest possible Blaze example: opens a window and lets the renderer
//! draw the built-in triangle. Hit Escape to quit.

use blaze_app::run;
use blaze_core::App;

fn main() {
    env_logger::init();
    let builder = App::builder();
    if let Err(e) = run(builder) {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
