mod shared_imports;

use shared_imports::*;

pub fn global_quit(siv_sink: &CbSink, shutdown_signal: &Arc<AtomicBool>) {
    // Set the shutdown signal
    shutdown_signal.store(true, Ordering::SeqCst);

    // Send a quit command to the UI
    let sink = siv_sink.clone();
    sink.send(Box::new(move |s| {
        s.quit();
    }))
    .unwrap_or(());

    // Spawn a thread that will force exit if clean shutdown takes too long
    std::thread::spawn(move || {
        // Give a shorter time for clean shutdown
        std::thread::sleep(Duration::from_millis(400));
        // Force exit the process
        std::process::exit(0);
    });
}
