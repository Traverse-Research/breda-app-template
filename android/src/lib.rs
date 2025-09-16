#![cfg(target_os = "android")]

use {{ template_code_friendly_id }}::*;

#[no_mangle]
fn android_main(app: AndroidApp) {
    internal_main(&CommandlineOpts::default(), app).unwrap()
}
