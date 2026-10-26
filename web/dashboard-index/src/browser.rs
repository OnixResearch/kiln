use std::cell::RefCell;

use rat_table::{DataTable, DataTableStyle};
use ratatui::{
    Terminal, TerminalOptions, Viewport,
    backend::Backend,
    layout::Rect,
    style::{Color, Style},
};
use ratzilla::DomBackend;
use wasm_bindgen::{JsCast, closure::Closure, prelude::*};
use web_sys::{
    CustomEvent, CustomEventInit, Document, Element, Event, EventTarget, HtmlElement, KeyboardEvent,
};

use crate::model::{Identity, Model, Movement, table_text};

const TABLE_ID: &str = "kiln-rats-table";
const NARROW_COLUMNS: u16 = 60;
const MAX_CELL_COLUMNS: u16 = 36;

type BrowserTerminal = Terminal<DomBackend>;

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

struct Listener {
    target: EventTarget,
    kind: &'static str,
    callback: Closure<dyn FnMut(Event)>,
}

impl Listener {
    fn new(
        target: EventTarget,
        kind: &'static str,
        handler: impl FnMut(Event) + 'static,
    ) -> Result<Self, JsValue> {
        let callback = Closure::wrap(Box::new(handler) as Box<dyn FnMut(Event)>);
        target.add_event_listener_with_callback(kind, callback.as_ref().unchecked_ref())?;
        Ok(Self {
            target,
            kind,
            callback,
        })
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        if let Err(error) = self
            .target
            .remove_event_listener_with_callback(self.kind, self.callback.as_ref().unchecked_ref())
        {
            web_sys::console::error_1(&error);
        }
    }
}

struct App {
    model: Model,
    table: DataTable,
    narrow_table: DataTable,
    style: DataTableStyle,
    terminal: Option<BrowserTerminal>,
    width: u16,
    height: u16,
    document: Document,
    grid: HtmlElement,
    status: Element,
    detail: Element,
    error: Element,
    activate: Element,
    listeners: Vec<Listener>,
}

fn js_error(error: impl std::fmt::Display) -> JsValue {
    js_sys::Error::new(&error.to_string()).into()
}

fn document() -> Result<Document, JsValue> {
    web_sys::window()
        .and_then(|window| window.document())
        .ok_or_else(|| js_error("Index requires a browser document"))
}

fn child(document: &Document, root: &Element, tag: &str) -> Result<Element, JsValue> {
    let element = document.create_element(tag)?;
    root.append_child(&element)?;
    Ok(element)
}

impl App {
    // r[impl kiln.dashboard.rats.widget]
    fn new(document: Document, root: Element, model: Model) -> Result<Self, JsValue> {
        let table = DataTable::new(
            vec![
                "Repository".into(),
                "History".into(),
                "Source".into(),
                "Revision".into(),
            ],
            model
                .entries
                .iter()
                .map(|entry| {
                    vec![
                        table_text(&entry.project_label),
                        table_text(&entry.history_label),
                        table_text(entry.source.as_deref().unwrap_or("—")),
                        table_text(entry.revision.as_deref().unwrap_or("—")),
                    ]
                })
                .collect(),
        )
        .with_min_cell_width(1)
        .with_max_cell_width(MAX_CELL_COLUMNS);
        let narrow_table = DataTable::new(
            vec!["Repository / history".into()],
            model
                .entries
                .iter()
                .map(|entry| {
                    vec![format!(
                        "{} / {}",
                        table_text(&entry.project_label),
                        table_text(&entry.history_label)
                    )]
                })
                .collect(),
        )
        .with_min_cell_width(1)
        .with_max_cell_width(MAX_CELL_COLUMNS);

        root.set_text_content(None);
        let style = child(&document, &root, "style")?;
        style.set_text_content(Some("#kiln-rats-table{box-sizing:border-box;width:100%;max-width:1280px;height:16rem;overflow:hidden;font:14px/normal monospace;background:#111827;color:#e5e7eb;border:1px solid #64748b}#kiln-rats-table:focus{outline:3px solid #38bdf8;outline-offset:2px}#kiln-rats-table pre{margin:0;padding:0;border:0;font:inherit}#rats-index .kiln-rats-detail,#rats-index .kiln-rats-status{white-space:pre-wrap;overflow-wrap:anywhere}#rats-index .kiln-rats-error{color:#b91c1c;white-space:pre-wrap;overflow-wrap:anywhere}"));
        let help = child(&document, &root, "p")?;
        help.set_id("kiln-rats-help");
        help.set_text_content(Some("Repository / history index · Subwayrat DataTable / Ratzilla. Focus the table: ↑/↓, Home/End choose a row; Enter opens it. Click a row to choose, then open the chosen history. Unicode table cells use ASCII escapes; full labels are below and in the HTML navigation."));
        let status = child(&document, &root, "p")?;
        status.set_class_name("kiln-rats-status");
        status.set_attribute("role", "status")?;
        let grid: HtmlElement = child(&document, &root, "div")?.dyn_into()?;
        grid.set_id(TABLE_ID);
        grid.set_tab_index(0);
        grid.set_attribute("role", "group")?;
        grid.set_attribute("aria-label", "Repository and history table")?;
        grid.set_attribute("aria-describedby", "kiln-rats-help kiln-rats-detail")?;
        let detail = child(&document, &root, "p")?;
        detail.set_id("kiln-rats-detail");
        detail.set_class_name("kiln-rats-detail");
        detail.set_attribute("aria-live", "polite")?;
        let activate = child(&document, &root, "button")?;
        activate.set_attribute("type", "button")?;
        activate.set_text_content(Some("Open chosen history"));
        let error = child(&document, &root, "p")?;
        error.set_class_name("kiln-rats-error");
        error.set_attribute("role", "alert")?;

        let mut app = Self {
            model,
            table,
            narrow_table,
            style: DataTableStyle {
                header_style: Style::default().fg(Color::Cyan).bold(),
                selected_style: Style::default().bg(Color::Blue).fg(Color::White).bold(),
                normal_style: Style::default().fg(Color::White),
                truncation_suffix: "...".into(),
                column_spacing: 1,
            },
            terminal: None,
            width: 0,
            height: 0,
            document,
            grid,
            status,
            detail,
            error,
            activate,
            listeners: Vec::new(),
        };
        app.resize()?;
        Ok(app)
    }

    fn resize(&mut self) -> Result<(), JsValue> {
        // Dropping the old backend unregisters its window-resize listener. A fixed
        // viewport avoids DomBackend::size(), which measures the global window.
        self.terminal.take();
        self.grid.set_text_content(None);
        let mut backend = DomBackend::new_by_id(TABLE_ID).map_err(js_error)?;
        let size = backend.window_size().map_err(js_error)?.columns_rows;
        self.width = size.width.max(1);
        self.height = size.height.max(1);
        self.terminal = Some(
            Terminal::with_options(
                backend,
                TerminalOptions {
                    viewport: Viewport::Fixed(Rect::new(0, 0, self.width, self.height)),
                },
            )
            .map_err(js_error)?,
        );
        // Cache each width-specific widget. Recalculate only on resize, never on
        // animation frames (there is deliberately no perpetual animation loop).
        let desktop_width = (self.width.saturating_sub(4) / 2).clamp(1, MAX_CELL_COLUMNS);
        self.table = std::mem::replace(&mut self.table, DataTable::new(Vec::new(), Vec::new()))
            .with_max_cell_width(desktop_width);
        self.narrow_table = std::mem::replace(
            &mut self.narrow_table,
            DataTable::new(Vec::new(), Vec::new()),
        )
        .with_max_cell_width(self.width.saturating_sub(1).clamp(1, MAX_CELL_COLUMNS));
        self.render()
    }

    // r[impl kiln.dashboard.rats.behavior]
    // r[impl kiln.dashboard.rats.behavior.scenario.restore]
    fn render(&mut self) -> Result<(), JsValue> {
        let active_exists = self.model.active_index().is_some();
        self.status.set_text_content(Some(&if active_exists {
            format!("Current report: {} / {}", self.model.active.project, self.model.active.history)
        } else {
            format!("Current selection is unavailable: {} / {}. No replacement was opened. Choose an available row explicitly to open another report.", self.model.active.project, self.model.active.history)
        }));
        if let Some(entry) = self.model.candidate() {
            self.detail.set_text_content(Some(&format!(
                "Chosen repository: {}\nHistory: {}\nIdentity: {} / {}\nSource: {}\nRevision: {}",
                entry.project_label,
                entry.history_label,
                entry.project,
                entry.history,
                entry.source.as_deref().unwrap_or("Not supplied"),
                entry.revision.as_deref().unwrap_or("Not supplied"),
            )));
            self.activate.remove_attribute("disabled")?;
        } else {
            self.detail
                .set_text_content(Some(if self.model.entries.is_empty() {
                    "No admitted histories are available."
                } else {
                    "No row chosen. Use an arrow key, Home/End, or click an available row."
                }));
            self.activate.set_attribute("disabled", "")?;
        }
        let table = if self.width < NARROW_COLUMNS {
            &mut self.narrow_table
        } else {
            &mut self.table
        };
        table.select_first();
        for _ in 0..self.model.cursor.unwrap_or(0) {
            table.select_next();
        }
        let selected_style = self.style.selected_style;
        if self.model.cursor.is_none() {
            self.style.selected_style = self.style.normal_style;
        }
        let result = self
            .terminal
            .as_mut()
            .ok_or_else(|| js_error("Index terminal is unavailable"))?
            .draw(|frame| table.render(frame, frame.area(), None, &self.style));
        self.style.selected_style = selected_style;
        result.map_err(js_error)?;
        // Mark real rendered data lines for pointer hit testing. The first line
        // is the widget header; empty space below the last row is not actionable.
        let terminal_grid = self
            .document
            .get_element_by_id(&format!("{TABLE_ID}_ratzilla_grid"))
            .ok_or_else(|| js_error("Ratzilla did not create its table grid"))?;
        terminal_grid.set_attribute("aria-hidden", "true")?;
        let start = self
            .model
            .visible_start(self.height.saturating_sub(1) as usize);
        let mut row = terminal_grid.first_element_child();
        let mut line = 0usize;
        while let Some(element) = row {
            if line > 0 && start + line - 1 < self.model.entries.len() {
                element.set_attribute("data-kiln-row", &(start + line - 1).to_string())?;
            } else {
                element.remove_attribute("data-kiln-row")?;
            }
            row = element.next_element_sibling();
            line += 1;
        }
        Ok(())
    }

    // r[impl kiln.dashboard.rats.widget.scenario.selection]
    fn install_listeners(&mut self) -> Result<(), JsValue> {
        self.listeners.push(Listener::new(
            self.grid.clone().into(),
            "keydown",
            |event| {
                let Some(key) = event.dyn_ref::<KeyboardEvent>() else {
                    return;
                };
                if key.alt_key() || key.ctrl_key() || key.meta_key() || key.is_composing() {
                    return;
                }
                let action = match key.key().as_str() {
                    "ArrowUp" => Some(Movement::Previous),
                    "ArrowDown" => Some(Movement::Next),
                    "Home" => Some(Movement::First),
                    "End" => Some(Movement::Last),
                    "Enter" => {
                        key.prevent_default();
                        activate();
                        return;
                    }
                    _ => None,
                };
                if let Some(action) = action {
                    key.prevent_default();
                    update(|app| {
                        app.model.move_cursor(action);
                        app.render()
                    });
                }
            },
        )?);
        self.listeners
            .push(Listener::new(self.grid.clone().into(), "click", |event| {
                let result = (|| {
                    let Some(target) = event
                        .target()
                        .and_then(|target| target.dyn_into::<Element>().ok())
                    else {
                        return Ok(());
                    };
                    let Some(row) = target.closest("[data-kiln-row]")? else {
                        return Ok(());
                    };
                    let Some(index) = row
                        .get_attribute("data-kiln-row")
                        .and_then(|value| value.parse::<usize>().ok())
                    else {
                        return Ok(());
                    };
                    update(|app| {
                        app.model.point(index);
                        app.grid.focus()?;
                        app.render()
                    });
                    Ok::<(), JsValue>(())
                })();
                if let Err(error) = result {
                    show_error(error);
                }
            })?);
        self.listeners.push(Listener::new(
            self.activate.clone().into(),
            "click",
            |_| activate(),
        )?);
        let window =
            web_sys::window().ok_or_else(|| js_error("Index requires a browser window"))?;
        self.listeners
            .push(Listener::new(window.into(), "resize", |_| {
                update(App::resize)
            })?);
        Ok(())
    }
}

fn show_error(error: JsValue) {
    let message = js_sys::Reflect::get(&error, &JsValue::from_str("message"))
        .ok()
        .and_then(|message| message.as_string())
        .or_else(|| error.as_string())
        .unwrap_or_else(|| format!("{error:?}"));
    APP.with(|state| {
        if let Some(app) = state.borrow().as_ref() {
            app.error.set_text_content(Some(&format!(
                "Repository index error: {message}. Use the HTML navigation instead."
            )));
        }
    });
    web_sys::console::error_1(&error);
}

fn update(action: impl FnOnce(&mut App) -> Result<(), JsValue>) {
    let result = APP.with(|state| {
        let mut state = state.borrow_mut();
        let app = state
            .as_mut()
            .ok_or_else(|| js_error("Repository index is not mounted"))?;
        action(app)
    });
    if let Err(error) = result {
        show_error(error);
    }
}

// r[impl kiln.dashboard.rats.widget.scenario.selection]
fn activate() {
    // Release the RefCell borrow before dispatch: the native bridge synchronously
    // calls select_history while handling this event.
    let identity = APP.with(|state| {
        state
            .borrow()
            .as_ref()
            .and_then(|app| app.model.candidate())
            .map(|entry| Identity {
                project: entry.project.clone(),
                history: entry.history.clone(),
            })
    });
    let Some(identity) = identity else { return };
    let result = (|| {
        let detail = js_sys::Object::new();
        js_sys::Reflect::set(&detail, &"project".into(), &identity.project.into())?;
        js_sys::Reflect::set(&detail, &"history".into(), &identity.history.into())?;
        let options = CustomEventInit::new();
        options.set_detail(&detail);
        let event = CustomEvent::new_with_event_init_dict("kiln-rats-select", &options)?;
        document()?.dispatch_event(&event)?;
        Ok::<(), JsValue>(())
    })();
    if let Err(error) = result {
        show_error(error);
    }
}

#[wasm_bindgen]
pub fn mount_index(entries_json: &str, project: &str, history: &str) -> Result<(), JsValue> {
    if APP.with(|state| state.borrow().is_some()) {
        return Err(js_error("Repository index is already mounted"));
    }
    let model = Model::parse(entries_json, project, history).map_err(js_error)?;
    let document = document()?;
    let root = document
        .get_element_by_id("rats-index")
        .ok_or_else(|| js_error("Missing #rats-index mount target"))?;
    let mut app = App::new(document, root, model)?;
    app.install_listeners()?;
    APP.with(|state| state.replace(Some(app)));
    Ok(())
}

#[wasm_bindgen]
pub fn select_history(project: &str, history: &str) {
    update(|app| {
        app.model.select_external(project, history);
        app.render()
    });
}
