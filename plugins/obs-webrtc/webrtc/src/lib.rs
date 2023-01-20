mod ffi;
mod obs_log;
mod output;
mod whip;

// Manually configure the linked runtimes for windows due to rust defaulting to msvcrt for both debug and release

#[cfg(all(not(debug_assertions), target_env = "msvc"))]
link_args::windows! {
    unsafe {
        default_lib("kernel32.lib", "vcruntime.lib", "msvcrtd", "ntdll.lib", "iphlpapi.lib");
        no_default_lib("msvcrtd", "vcruntimed.lib");
    }
}

#[cfg(all(debug_assertions, target_env = "msvc"))]
link_args::windows! {
    unsafe {
        default_lib("kernel32.lib", "vcruntimed.lib", "msvcrtd", "ntdll.lib", "iphlpapi.lib");
        no_default_lib("msvcrt", "vcruntime.lib");
    }
}
