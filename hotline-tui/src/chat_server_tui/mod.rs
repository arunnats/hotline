mod imports;
mod utils;

pub use imports::*;
pub use utils::*;

pub fn run_server_tui() {
    // Create a shared shutdown signal
    let shutdown_signal = Arc::new(AtomicBool::new(false));

    // Create standard synchronous channels for UI to communicate with the async thread
    let (ui_to_async_tx, ui_to_async_rx) = std_mpsc::channel::<String>();
    let (async_to_ui_tx, async_to_ui_rx) = std_mpsc::channel::<OutputEvent>();

    // Clone the shutdown signal for the async thread
    let thread_shutdown_signal = Arc::clone(&shutdown_signal);

    // Spawn the async thread with its own Tokio runtime
    let async_thread: thread::JoinHandle<()> = thread::spawn(move || {
        // Create a new Tokio runtime in this thread
        let rt = Runtime::new().unwrap();

        // Run the async code in this runtime
        rt.block_on(async {
            // Create Tokio channels for the async code
            let (input_tx, input_rx) = mpsc::channel::<String>(100);
            let (output_tx, mut output_rx) = mpsc::channel::<OutputEvent>(100);

            // Bridge between std_mpsc and tokio channels
            let input_tx_clone = input_tx.clone();
            let input_shutdown = Arc::clone(&thread_shutdown_signal);
            let input_handle = tokio::spawn(async move {
                while let Ok(message) = ui_to_async_rx.recv() {
                    if input_shutdown.load(Ordering::SeqCst) {
                        break;
                    }
                    if input_tx_clone.send(message).await.is_err() {
                        break;
                    }
                }
            });

            let output_shutdown = Arc::clone(&thread_shutdown_signal);
            let async_to_ui_tx_clone = async_to_ui_tx.clone();
            let output_handle = tokio::spawn(async move {
                while let Some(event) = output_rx.recv().await {
                    if output_shutdown.load(Ordering::SeqCst) {
                        break;
                    }
                    if async_to_ui_tx_clone.send(event).is_err() {
                        break;
                    }
                }
            });

            // Run the server backend
            match run_server_backend(input_rx, output_tx, thread_shutdown_signal).await {
                Ok(_) => {
                    // Server stopped normally
                }
                Err(e) => {
                    // Send error to UI
                    eprintln!("Server error: {}", e);
                    let _ = async_to_ui_tx.send(OutputEvent::TextLine(TextLine {
                        text: format!("Server error: {}\n", e),
                        color: Some(RED_COLOR.clone()),
                    }));
                    let _ = async_to_ui_tx.send(OutputEvent::SystemEvent(
                        SystemEvent::ConnectionError {
                            message: format!("Server error: {}", e),
                        },
                    ));
                }
            }

            let _ = input_handle.await;
            let _ = output_handle.await;
        });
    });

    // Run the UI in the main thread
    server_tui(ui_to_async_tx, async_to_ui_rx, shutdown_signal.clone());

    // Wait for the async thread to finish
    let _ = async_thread.join();
}
