// A window, not a console: Windows must not open a terminal behind the application.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    assistant_cabinet_ai_lib::run()
}
