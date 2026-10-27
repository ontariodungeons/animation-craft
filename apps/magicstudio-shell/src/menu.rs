//! Native application menu; project actions retain the existing frontend/CLI guards.
use tauri::{Manager, menu::{MenuBuilder, MenuItemBuilder, SubmenuBuilder}};

pub fn install(window: &tauri::Window) -> tauri::Result<()> {
    let new = MenuItemBuilder::with_id("file.new", "New Project…").accelerator("CmdOrCtrl+N").build(window)?;
    let open = MenuItemBuilder::with_id("file.open", "Open Project…").accelerator("CmdOrCtrl+O").build(window)?;
    let save = MenuItemBuilder::with_id("file.save", "Save Project").accelerator("CmdOrCtrl+S").build(window)?;
    let close = MenuItemBuilder::with_id("file.close", "Close Project").accelerator("CmdOrCtrl+W").build(window)?;
    let settings = MenuItemBuilder::with_id("app.settings", "Settings…").accelerator("CmdOrCtrl+,").build(window)?;
    let quit = MenuItemBuilder::with_id("app.quit", "Quit Magic Studio").accelerator("CmdOrCtrl+Q").build(window)?;
    let file = SubmenuBuilder::new(window, "File")
        .items(&[&new, &open, &save, &close]).separator()
        .item(&settings).separator().item(&quit).build()?;
    let edit = SubmenuBuilder::new(window, "Edit").undo().redo().separator()
        .cut().copy().paste().select_all().build()?;
    window.set_menu(MenuBuilder::new(window).items(&[&file, &edit]).build()?)?;
    window.on_menu_event(|window, event| {
        if event.id().as_ref() == "app.quit" {
            // close(), unlike app.exit(), goes through the existing CloseRequested guard.
            if let Err(error) = window.close() { eprintln!("menu quit: {error}"); }
            return;
        }
        if let Some(view) = window.get_webview("project") {
            let id = serde_json::json!(event.id().as_ref());
            if let Err(error) = view.eval(format!("window.studioMenu({id});")) {
                eprintln!("native menu: {error}");
            }
        }
    });
    Ok(())
}
