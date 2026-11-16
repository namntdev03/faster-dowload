#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod jobs;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(jobs::Jobs::default())
        .invoke_handler(tauri::generate_handler![
            commands::default_dir,
            commands::add_download,
            commands::list_jobs,
            commands::pause_job,
            commands::resume_job,
            commands::remove_job,
            commands::reveal_job,
        ])
        .run(tauri::generate_context!())
        .expect("lỗi khi chạy ứng dụng FastDL");
}
