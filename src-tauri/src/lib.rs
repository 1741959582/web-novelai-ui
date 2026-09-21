mod nai;
mod nai_stream;
mod png_meta;
mod store;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            store::settings_get,
            store::settings_save,
            store::history_list,
            store::history_delete,
            store::account_get,
            store::pick_output_dir,
            store::open_output_dir,
            store::read_image_data_url,
            store::sessions_list,
            store::session_upsert,
            store::session_load,
            store::session_delete,
            store::session_new,
            nai::token_verify,
            nai::account_refresh,
            nai::generate_txt2img,
            nai::generate_img2img,
            png_meta::inspect_image,
            png_meta::inspect_image_bytes,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
