//! Loaded into GUI apps: makes the color in the settings the system accent, in that app.
//!
//! The `custommaccolors` daemon maps this dylib into apps with eris-inject; during development it's
//! just `dlopen`ed (see `tools/accenthost.m`). Either way it runs on eris-inject-payload's `no_std`
//! runtime, which is what makes it safe inside arm64e apps.
#![no_std]

extern crate alloc;

pub(crate) use eris_inject_payload::{log, msg, objc, rt};

mod accent;
mod settings;

eris_inject_payload::payload!(name: "custommaccolors", log: "CustomMacColors", start: start);

/// A class every copy of us defines, so a copy can tell another is already here.
const LOADED_MARKER: &core::ffi::CStr = c"CustomMacColorsLoaded";

fn start(injected: bool) {
    let pid = unsafe { rt::getpid() };
    // The daemon loads us as soon as an app starts, which can be before AppKit is up.
    let wait_for_appkit_ms = if injected { 10_000 } else { 0 };
    let mut waited = 0;
    while objc::class(c"NSApplication").is_null() {
        if waited >= wait_for_appkit_ms {
            log!("no AppKit in pid {pid}; nothing to do");
            return;
        }
        unsafe { rt::usleep(50_000) };
        waited += 50;
    }
    // Only processes that become apps have an accent; plenty of others link AppKit and never do.
    // The daemon starts us while the app is still starting up, so give it a while.
    // (Loaded by dyld, we're on the main thread before the app exists; the main queue waits for it.)
    let mut waited = 0;
    while injected && !has_app() {
        if waited >= 30_000 {
            log!("no NSApplication in pid {pid}; nothing to do");
            return;
        }
        unsafe { rt::usleep(100_000) };
        waited += 100;
    }
    eris_inject_payload::sys::on_main(install);
}

/// Whether AppKit's `NSApp` has been made.
fn has_app() -> bool {
    unsafe extern "C" {
        fn dlsym(handle: *mut core::ffi::c_void, symbol: *const core::ffi::c_char) -> *mut core::ffi::c_void;
    }
    const RTLD_DEFAULT: *mut core::ffi::c_void = -2isize as *mut core::ffi::c_void;
    let app = unsafe { dlsym(RTLD_DEFAULT, c"NSApp".as_ptr()) } as *const objc::Id;
    !app.is_null() && !unsafe { core::ptr::read_volatile(app) }.is_null()
}

extern "C" fn install(_: *mut core::ffi::c_void) {
    if objc::define_class(LOADED_MARKER, c"NSObject", &[]).is_null() {
        log!("already here in pid {}", unsafe { rt::getpid() });
        return;
    }
    objc::autoreleasing(settings::listen);
}
