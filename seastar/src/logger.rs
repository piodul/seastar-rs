use std::fmt::Arguments;
use std::pin::Pin;

use cxx::UniquePtr;
use once_cell::sync::OnceCell;

#[cxx::bridge(namespace = "seastar_ffi::logger")]
mod ffi {
    extern "Rust" {
        type FormatCtx<'a>;
        fn write_log_line(writer: Pin<&mut log_writer>, ctx: &FormatCtx<'_>);
    }

    unsafe extern "C++" {
        include!("seastar/src/logger.hh");

        type log_writer;
        fn write(self: Pin<&mut log_writer>, data: &[u8]);

        type logger;
        fn new_logger(name: &str) -> UniquePtr<logger>;
        fn log<'a>(l: &logger, level: u32, ctx: &FormatCtx<'_>);
    }
}

pub struct FormatCtx<'a> {
    args: Arguments<'a>,
}

fn write_log_line(writer: Pin<&mut ffi::log_writer>, ctx: &FormatCtx<'_>) {
    struct FmtWriter<'a>(Pin<&'a mut ffi::log_writer>);
    impl<'a> std::fmt::Write for FmtWriter<'a> {
        fn write_str(&mut self, s: &str) -> std::fmt::Result {
            self.0.as_mut().write(s.as_bytes());
            Ok(())
        }
    }

    std::fmt::write(&mut FmtWriter(writer), ctx.args).unwrap();
}

#[repr(u32)]
pub enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

pub struct Logger {
    core: OnceCell<UniquePtr<ffi::logger>>,
    name: &'static str,
}

unsafe impl Send for Logger {}
unsafe impl Sync for Logger {}

impl Logger {
    #[inline]
    pub const fn new(name: &'static str) -> Self {
        Self {
            core: OnceCell::new(),
            name,
        }
    }

    #[inline]
    pub fn log(&self, level: LogLevel, args: Arguments<'_>) {
        let core = self.core.get_or_init(|| ffi::new_logger(self.name));
        let ctx = FormatCtx { args };
        ffi::log(&core, level as u32, &ctx);
    }

    #[inline]
    pub fn trace(&self, args: Arguments<'_>) {
        self.log(LogLevel::Trace, args);
    }

    #[inline]
    pub fn debug(&self, args: Arguments<'_>) {
        self.log(LogLevel::Debug, args);
    }

    #[inline]
    pub fn info(&self, args: Arguments<'_>) {
        self.log(LogLevel::Info, args);
    }

    #[inline]
    pub fn warn(&self, args: Arguments<'_>) {
        self.log(LogLevel::Warn, args);
    }

    #[inline]
    pub fn error(&self, args: Arguments<'_>) {
        self.log(LogLevel::Error, args);
    }
}

#[macro_export]
macro_rules! trace {
    ($logger:expr, $($arg:tt),*) => {{
        $logger.trace(std::format_args!($($arg),*))
    }};
}

#[macro_export]
macro_rules! debug {
    ($logger:expr, $($arg:tt),*) => {{
        $logger.debug(std::format_args!($($arg),*))
    }};
}

#[macro_export]
macro_rules! info {
    ($logger:expr, $($arg:tt),*) => {{
        $logger.info(std::format_args!($($arg),*))
    }};
}

#[macro_export]
macro_rules! warn {
    ($logger:expr, $($arg:tt),*) => {{
        $logger.warn(std::format_args!($($arg),*))
    }};
}

#[macro_export]
macro_rules! error {
    ($logger:expr, $($arg:tt),*) => {{
        $logger.error(std::format_args!($($arg),*))
    }};
}
