use env_logger::filter::{Builder, Filter};
use log::{Level, Log};
use std::{ffi::CStr, sync::atomic::AtomicBool, sync::atomic::Ordering};

static DEBUG_WHIP: AtomicBool = AtomicBool::new(false);

pub(crate) fn debug_whip() -> bool {
    DEBUG_WHIP.load(Ordering::Relaxed)
}

/// cbindgen:ignore
mod obs {
    use std::os::raw::c_char;
    use std::os::raw::c_int;

    #[repr(C)]
    #[derive(Debug)]
    pub(super) struct obs_cmdline_args {
        pub(super) argc: c_int,
        pub(super) argv: *const *const c_char,
    }

    #[link(name = "libobs")]
    extern "C" {
        pub(super) fn blog(log_level: i32, format: *const c_char, ...);
        pub(super) fn obs_get_cmdline_args() -> obs_cmdline_args;
    }
}

pub struct ObsLogger {
    log_filter: Filter,
}

impl ObsLogger {
    pub fn install() {
        // Pull our RUST_LOG in case they are using that
        let mut log_filter: String = std::env::var("RUST_LOG").unwrap_or_else(|_| "INFO".into());

        unsafe {
            let raw_args = obs::obs_get_cmdline_args();
            let mut args = std::slice::from_raw_parts(raw_args.argv, raw_args.argc as usize)
                .iter()
                .map(|arg| CStr::from_ptr(*arg).to_string_lossy());

            while let Some(arg) = args.next() {
                match arg.as_ref() {
                    "--debug-webrtc-whip" => {
                        DEBUG_WHIP.store(true, Ordering::Relaxed);
                    }
                    "--debug-webrtc-filter" => {
                        if let Some(filter) = args.next() {
                            log_filter = filter.into();
                        }
                    }
                    _ => {}
                }
            }
            let mut filter_builder = Builder::new();
            log::set_boxed_logger(Box::new(Self {
                log_filter: filter_builder.parse(&log_filter).build(),
            }))
            .expect("Logger already set");

            log::set_max_level(log::LevelFilter::Trace);
        }
    }
}

impl Log for ObsLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        self.log_filter.enabled(metadata)
    }

    fn log(&self, record: &log::Record) {
        if !self.log_filter.matches(record) {
            return;
        }

        let level = record.level();
        let log_level = match level {
            Level::Trace => (300, "[T] "),
            Level::Debug => (300, "[D] "),
            Level::Info => (300, ""),
            Level::Warn => (200, ""),
            Level::Error => (100, ""),
        };

        let message_prefix = format!(
            "{}[{}:{}]",
            log_level.1,
            if record.target().is_empty() {
                record.module_path().unwrap_or("unk")
            } else {
                record.target()
            },
            record.line().unwrap_or(0),
        );

        const MAX_CHUNK_SIZE: usize = 3500;

        let message_chars = record.args().to_string().chars().collect::<Vec<char>>();
        let chunks = message_chars.chunks(MAX_CHUNK_SIZE);
        let chunks_len = chunks.len();
        for (index, chunk) in chunks.enumerate() {
            unsafe {
                let chunk = chunk.iter().cloned().collect::<String>();
                if chunks_len == 1 {
                    obs::blog(
                        log_level.0,
                        format!("{message_prefix}: {chunk}\0").as_ptr() as *const i8,
                    );
                } else {
                    obs::blog(
                        log_level.0,
                        format!(
                            "{message_prefix}: MULTIPART [{}/{chunks_len}] {chunk}\0",
                            index + 1,
                        )
                        .as_ptr() as *const i8,
                    );
                }
            }
        }
    }

    fn flush(&self) {}
}
