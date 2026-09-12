// Knoasis Tauri 入口：注册 grammar 只读命令 + userdata 命令
// 数据层见 docs/Knoasis-韩语语法接入设计.md §2 / §4.5；多学科扩展见 docs/Knoasis-英语语法接入设计.md

mod grammar;
mod knowledge;
mod userdata;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        // 学科包管理：导入学科包时用原生目录选择器
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            // userdata.db 可写单连接：启动即建目录 + 建表 + 迁移（v2→v3），供 user_* 命令使用
            let conn = userdata::open_userdata(app.handle())?;
            app.manage(userdata::UserData(std::sync::Mutex::new(conn)));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            grammar::grammar_list_sets,
            grammar::grammar_list_entries,
            grammar::grammar_get_detail,
            grammar::grammar_reload_db,
            knowledge::knowledge_list_sets,
            knowledge::knowledge_list_entries,
            knowledge::knowledge_get_entry,
            knowledge::knowledge_reload,
            knowledge::knowledge_manage_list,
            knowledge::knowledge_set_enabled,
            knowledge::knowledge_import_set,
            knowledge::knowledge_remove_set,
            knowledge::knowledge_open_dir,
            userdata::user_favorites_list,
            userdata::user_favorites_add,
            userdata::user_favorites_remove,
            userdata::user_notes_list,
            userdata::user_notes_save,
            userdata::user_migrate_legacy,
            userdata::user_image_hide,
            userdata::user_image_restore,
            userdata::user_review_set,
            userdata::user_review_get,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
