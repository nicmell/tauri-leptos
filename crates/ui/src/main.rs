mod app;

use app::App;
use leptos::prelude::*;
use tracing_subscriber::prelude::*;
use tracing_web::MakeWebConsoleWriter;

fn init_tracing() {
    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_ansi(false)
        .without_time()
        .with_writer(MakeWebConsoleWriter::new());
    tracing_subscriber::registry().with(fmt_layer).init();
}

fn main() {
    console_error_panic_hook::set_once();
    init_tracing();
    tracing::info!("ui starting");
    mount_to_body(|| {
        view! { <App /> }
    });
}
