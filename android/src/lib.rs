#![cfg(target_os = "android")]

use app_template::*;

#[no_mangle]
fn android_main(app: AndroidApp) {
    internal_main(&CommandlineOpts::default(), app).unwrap()
}
