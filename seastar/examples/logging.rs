use seastar::{AppTemplate, Logger};

static LOGGER: Logger = Logger::new("test_logger");
static LOGGER2: Logger = Logger::new("my_little_logger");

pub fn main() {
    let mut template = AppTemplate::default();
    template.run_void(std::env::args(), async {
        seastar::info!(LOGGER, "Hello, world!");
        seastar::warn!(LOGGER2, "Oh no! {}", 123);
        Ok(())
    });
}
