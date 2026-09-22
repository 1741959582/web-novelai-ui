mod apng;
mod cl_tagger_local;
mod danbooru;
mod gpu;
mod nai;
mod nai_stream;
mod png_meta;
mod quicktag;
mod reference_presets;
mod reverse_tasks;
mod store;
mod update;
mod wd_tagger;

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            store::settings_get,
            store::settings_save,
            store::history_list,
            store::history_delete,
            store::history_groups_list,
            store::history_group_create,
            store::history_group_rename,
            store::history_group_delete,
            store::history_set_group,
            store::reveal_in_folder,
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
            nai::fetch_remote_image,
            nai::translate_text,
            danbooru::danbooru_status,
            danbooru::download_danbooru,
            danbooru::suggest_tags,
            danbooru::lookup_tags,
            danbooru::add_custom_tag,
            wd_tagger::wd_tag_image,
            wd_tagger::wd_tag_cancel,
            gpu::gpu_detect,
            cl_tagger_local::cl_tagger_status,
            cl_tagger_local::cl_tagger_download,
            cl_tagger_local::cl_tagger_open_dir,
            reverse_tasks::reverse_tasks_list,
            reverse_tasks::reverse_task_current,
            reverse_tasks::reverse_task_save,
            reverse_tasks::reverse_task_load,
            reverse_tasks::reverse_task_delete,
            reverse_tasks::reverse_task_new,
            quicktag::quicktag_catalog,
            quicktag::quicktag_search,
            quicktag::quicktag_entry,
            reference_presets::reference_preset_list,
            reference_presets::reference_preset_save,
            reference_presets::reference_preset_delete,
            reference_presets::reference_catalog_load,
            reference_presets::reference_catalog_download,
            nai::generate_txt2img,
            nai::generate_img2img,
            nai::upscale_image,
            nai::augment_image,
            apng::apng_disguise,
            apng::apng_gif,
            apng::apng_clean,
            apng::apng_strip,
            apng::apng_mosaic,
            apng::apng_restore,
            apng::pick_images,
            apng::copy_image_files,
            update::app_version,
            update::check_app_update,
            update::open_latest_release,
            update::install_app_update,
            png_meta::inspect_image,
            png_meta::inspect_image_bytes,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
