use conrod::backend::glium::{glium, glium::glutin, Renderer as GliumRenderer};
use vst::editor::Editor;

use conrod::widget_ids;
use conrod::color::hsl;
use conrod::utils::degrees;

use std::path::PathBuf;
use std::sync::mpsc;

use crate::AudioCommand;

const WINDOW_SIZE: (u32, u32) = (110, 200);

struct Window {
    ui: conrod::Ui,
    ids: Ids,

    image_map: conrod::image::Map<glium::texture::Texture2d>,

    event_loop: glutin::EventsLoop,
    display: glium::Display,
    renderer: GliumRenderer,

    command_tx: mpsc::Sender<AudioCommand>,

    file_hovered: bool,
}


impl Window {
    fn new(command_tx: mpsc::Sender<AudioCommand>) -> Self {
        let image_map = conrod::image::Map::new();
        let mut ui = conrod::UiBuilder::new([WINDOW_SIZE.0 as f64, WINDOW_SIZE.1 as f64])
            .theme(theme())
            .build();

        const FONT_BYTES: &'static [u8] = include_bytes!("../assets/Quirk.ttf");
        let font = conrod::text::Font::from_bytes(FONT_BYTES).unwrap();
        ui.fonts.insert(font);

        let ids = Ids::new(ui.widget_id_generator());

        let event_loop = glutin::EventsLoop::new();
        let window = glutin::WindowBuilder::new()
            .with_dimensions(WINDOW_SIZE.into())
            .with_always_on_top(true)
            // .with_decorations(false)
            .with_resizable(false)
            .with_title("WOMP");

        let context = glutin::ContextBuilder::new().with_srgb(true);
        let display = glium::Display::new(window, context, &event_loop).unwrap();

        let renderer = conrod::backend::glium::Renderer::new(&display).unwrap();

        Window {
            ui, ids,
            image_map,

            event_loop,
            display,
            renderer,

            command_tx,
            file_hovered: false,
        }
    }

    fn update(&mut self) {
        let mut events = Vec::new();

        self.event_loop.poll_events(|e| events.push(e));

        for event in events {
            use glutin::{Event, WindowEvent};

            match event {
                Event::WindowEvent{ event: WindowEvent::HoveredFile(_), .. } => {
                    self.file_hovered = true;
                }
                Event::WindowEvent{ event: WindowEvent::DroppedFile(filename), .. } => {
                    if let Ok(src) = std::fs::read_to_string(filename) {
                        self.command_tx.send(AudioCommand::SetModel(src)).unwrap();
                    }

                    self.file_hovered = false;
                }
                Event::WindowEvent{ event: WindowEvent::HoveredFileCancelled, .. } => {
                    self.file_hovered = false;
                }

                e => if let Some(event) = conrod::backend::winit::convert_event(e, &self.display) {
                    self.ui.handle_event(event);
                }
            }
        }

        self.set_widgets();

        // Render the `Ui` and then display it on the screen.
        if let Some(primitives) = self.ui.draw_if_changed() {
            use glium::Surface;

            let mut target = self.display.draw();
            target.clear_color(0.03, 0.03, 0.03, 0.0);
            self.renderer.fill(&self.display, primitives, &self.image_map);
            self.renderer.draw(&self.display, &mut target, &self.image_map).unwrap();
            target.finish().unwrap();
        }
    }

    fn set_widgets(&mut self) {
        let ui = &mut self.ui.set_widgets();
        let ids = &self.ids;

        use conrod::*;
        use conrod::widget::*;
        use conrod::position::*;

        // let canvas_split = 2.0;
        // let canvas_offset = -1.0;

        // let canvas_color = hsl(degrees(160.0), 0.3, 0.6);

        let canvas_width = WINDOW_SIZE.0 as f64 - 20.0;
        let canvas_height = WINDOW_SIZE.1 as f64 - 20.0;

        Canvas::new()
            // .color(canvas_color)
            .w_h(canvas_width, canvas_height)
            // .x_y(canvas_offset+canvas_split, canvas_offset+canvas_split)
            .set(ids.canvas, ui);

        let export_button = Button::new()
            .mid_top_of(ids.canvas)
            .label("export")
            .w_h(100.0, 20.0)
            .set(ids.export_button, ui);

        if export_button.was_clicked() {            
            
        }

        let edit_button = Button::new()
            .label("edit")
            .w_h(100.0, 20.0)
            .set(ids.edit_button, ui);

        if edit_button.was_clicked() {
            // ctx.about = !ctx.about;
        }

        let load_default_button = Button::new()
            .label("load default")
            .w_h(100.0, 20.0)
            .set(ids.load_default_button, ui);

        if load_default_button.was_clicked() {
            let source = include_str!("../assets/default.lisp").into();
            self.command_tx.send(AudioCommand::SetModel(source)).unwrap();
        }

        let load_sound_test_button = Button::new()
            .label("load test")
            .w_h(100.0, 20.0)
            .set(ids.load_sound_test_button, ui);

        if load_sound_test_button.was_clicked() {
            let source = include_str!("../assets/sound_test.lisp").into();
            self.command_tx.send(AudioCommand::SetModel(source)).unwrap();
        }

        if self.file_hovered {
            Canvas::new()
                .color(hsl(degrees(160.0), 0.3, 0.6))
                .w_h(canvas_width, canvas_height)
                .set(ids.import_drop_zone, ui);
        }
    }
}

widget_ids! {
    struct Ids {
        canvas,
        export_button,
        edit_button,

        load_default_button,
        load_sound_test_button,

        import_drop_zone,
    }
}


impl Drop for Window {
    fn drop(&mut self) {
        self.display.gl_window().hide();
    }
}



pub struct View {
    window: Option<Window>,
    command_tx: mpsc::Sender<AudioCommand>,
}

impl View {
    pub fn new(command_tx: mpsc::Sender<AudioCommand>) -> View {
        View {
            window: None,
            command_tx,
        }
    }
}


use std::ffi::c_void;

impl Editor for View {
    fn size(&self) -> (i32, i32) { (200, 100) }
    fn position(&self) -> (i32, i32) { (0, 0) }

    fn open(&mut self, _: *mut c_void) {
        self.window = Some(Window::new(self.command_tx.clone()));
    }

    fn close(&mut self) {
        self.window = None;
    }

    fn is_open(&mut self) -> bool { self.window.is_some() }

    fn idle(&mut self) {
        if let Some(window) = &mut self.window {
            window.update();
        }
    }
}


fn theme() -> conrod::Theme {
    use conrod::*;
    use conrod::position::{Padding, Direction, Position, Relative};

    Theme {
        border_width: 0.0,

        font_size_large: 20,
        font_size_medium: 12,
        font_size_small: 8,

        shape_color: hsl(degrees(80.0), 0.3, 0.7),
        label_color: hsl(degrees(0.0), 0.6, 0.6),

        y_position: Position::Relative(Relative::Direction(Direction::Backwards, 10.0), None),

        padding: Padding {
            x: Range::new(5.0, 5.0),
            y: Range::new(5.0, 5.0),
        },

        .. Theme::default()
    }
}
