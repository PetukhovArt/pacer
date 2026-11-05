mod pacer;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(pacer::Pacer::spawn())
        .invoke_handler(tauri::generate_handler![
            pacer::pacer_watch,
            pacer::pacer_attach,
            pacer::pacer_input,
            pacer::pacer_resize,
            pacer::pacer_detach,
            pacer::pacer_mark_seen,
            pacer::pacer_call,
        ])
        .run(tauri::generate_context!())
        .expect("error while running veyra desktop");
}
