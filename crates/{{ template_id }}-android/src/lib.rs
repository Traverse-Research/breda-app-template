#![cfg(target_os = "android")]

use testing_package::*;

#[unsafe(no_mangle)]
fn android_main(app: AndroidApp) {
    internal_main(&CommandlineOpts::default(), app).unwrap()
}
