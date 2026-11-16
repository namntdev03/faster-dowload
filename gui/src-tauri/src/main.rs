#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod job;
mod jobs;

use tauri::Manager;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(jobs::Jobs::new())
        .setup(|app| {
            let dir = app.path().app_data_dir()?;
            app.state::<jobs::Jobs>().open(dir.join("jobs.json"));
            Ok(())
        })
        .on_window_event(|win, ev| {
            if let tauri::WindowEvent::Destroyed = ev {
                win.state::<jobs::Jobs>().save();
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::default_dir,
            commands::add_download,
            commands::list_jobs,
            commands::pause_job,
            commands::resume_job,
            commands::remove_job,
            commands::reveal_job,
            commands::set_max_active,
        ])
        .run(tauri::generate_context!())
        .expect("lỗi khi chạy ứng dụng FastDL");
}
