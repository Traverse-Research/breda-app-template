use crate::CommandlineOpts;
use clap::Parser;

#[cfg(target_os = "android")]
#[unsafe(no_mangle)]
fn android_main(app: android_activity::AndroidApp) {
    if let Err(err) = crate::internal_main(&CommandlineOpts::parse(), app) {
        log::error!("evolve exited with failure: {err:?}");
    }
}

