#![feature(box_syntax)]
#![feature(fixed_size_array)]

use vst::plugin::{Info, CanDo, Plugin, HostCallback, Category as VstCategory};
use vst::editor::Editor;
use vst::buffer::AudioBuffer as VstAudioBuffer;
use vst::api::Supported as VstSupported;
use vst::api::Events as VstEvents;

use std::sync::mpsc;

mod model;
mod view;
mod lisp;
mod voice_allocator;

use self::view::View;
use self::model::Model;

// use model::WonkModel as Model;
// use model::WonkParameter as Parameter;

pub type VstResult<T> = Result<T, failure::Error>;

use std::sync::Once;
static LOGGER_INIT: Once = Once::new();

pub enum AudioCommand {
    SetModel(String),
}

struct BasicPlugin {
    view: View,
    model: Option<Model>,

    audio_cmd_rx: mpsc::Receiver<AudioCommand>,

    synth_ctx: voi_synth::Context,
}

impl Plugin for BasicPlugin {
    fn get_info(&self) -> Info {
        log::info!("get_info");

        Info {
            name: "Lisp Synth".to_string(),
            vendor: "_manpat".to_string(),
            unique_id: 20190420,

            category: VstCategory::Synth,

            outputs: 1,

            preset_chunks: true,

            ..Info::default()
        }
    }

    fn get_editor(&mut self) -> Option<&mut dyn Editor> { Some(&mut self.view) }

    fn can_do(&self, can_do: CanDo) -> VstSupported {
        match can_do {
            CanDo::ReceiveMidiEvent => VstSupported::Yes,
            _ => VstSupported::Maybe,
        }
    }

    fn set_block_size(&mut self, size: i64) { self.synth_ctx.set_buffer_size(size as _) }
    fn set_sample_rate(&mut self, rate: f32) { self.synth_ctx.set_sample_rate(rate) }


    // fn get_preset_num(&self) -> i32 { log::info!("get_preset_num"); 0 }
    // fn get_preset_name(&self, preset: i32) -> String {
    //     log::info!("get_preset_name {}", preset);
    //     format!("preset {}", preset)
    // }

    // fn set_preset_name(&mut self, name: String) { log::info!("set_preset_name {}", name) }


    // fn get_preset_data(&mut self) -> Vec<u8> { log::info!("get_preset_data"); Vec::new() }
    fn get_bank_data(&mut self) -> Vec<u8> {
        if let Some(ref model) = self.model {
            model.source.as_bytes().into()
        } else {
            Vec::new()
        }
    }

    // fn load_preset_data(&mut self, data: &[u8]) { log::info!("load_preset_data"); }
    fn load_bank_data(&mut self, data: &[u8]) {
        match String::from_utf8(data.to_owned()) {
            Ok(source) => self.load_model(source),
            Err(_) => log::error!("load_bank_data got invalid data")
        }
    }

    // fn get_parameter_name(&self, index: i32) -> String {
    //     Parameter::from_index(index)
    //         .as_ref()
    //         .map(Parameter::get_name)
    //         .unwrap_or_else(|| String::new())
    // }

    // fn get_parameter_text(&self, index: i32) -> String {
    //     if let Some(param) = Parameter::from_index(index) {
    //         self.model.get_parameter(param)
    //             .unwrap()
    //             .to_string()

    //     } else {
    //         String::new()
    //     }
    // }

    // fn can_be_automated(&self, index: i32) -> bool {
    //     index < Parameter::num_display_params()
    // }

    // fn get_parameter(&self, index: i32) -> f32 {
    //     Parameter::from_index(index)
    //         .and_then(|p| {
    //             let (min, max) = p.get_min_max();
    //             let diff = max - min;

    //             self.model.get_parameter(p)
    //                 .map(|v| (v - min) / diff)
    //         })
    //         .unwrap_or(0.0)
    // }

    // fn set_parameter(&mut self, index: i32, val: f32) {
    //     if let Some(p) = Parameter::from_index(index) {
    //         let (min, max) = p.get_min_max();
    //         let diff = max - min;

    //         let val = val * diff + min;

    //         self.model.set_parameter(p, val);
    //         self.synth_view.model.set_parameter(p, val);
            
    //         // NOTE: this is too often
    //         // self.synth_ctx.set_parameter(self.model.parameters[index as usize], val);
    //     }
    // }

    fn process(&mut self, out_buf: &mut VstAudioBuffer<f32>) {
        assert!(out_buf.output_count() == 1);

        self.process_audio_commands();

        let buf = self.synth_ctx.get_ready_buffer().expect("Failed to get ready buffer");

        if buf.len() == out_buf.samples() {
            let out_buf = out_buf.split().1.get_mut(0);
            buf.copy_to(out_buf);
        } else {
            log::warn!("Buffer size mismatch in plugin process");
        }

        self.synth_ctx.queue_empty_buffer(buf).unwrap();
    }

    fn process_events(&mut self, events: &VstEvents) {
        use vst::event::Event;

        for event in events.events() {
            match event {
                Event::Midi(midi_event) => self.process_midi_event(midi_event),
                _ => {}
            }
        }
    }
}


impl Default for BasicPlugin {
    fn default() -> Self {
        use std::fs;
        
        LOGGER_INIT.call_once(|| {
            use simplelog::*;
            use std::fs::File;

            std::env::set_var("RUST_BACKTRACE", "1");

            let log_dir = dirs::data_dir().unwrap().join("_manpat");

            fs::create_dir_all(&log_dir).unwrap();

            if let Ok(file) = File::create(log_dir.join("vst-lisp.log")) {
                WriteLogger::init(
                    LevelFilter::Info,
                    Config::default(),
                    file
                ).unwrap();

                log_panics::init();
            }

            log::info!("Logging enabled");
        });

        let (audio_cmd_tx, audio_cmd_rx) = mpsc::channel();

        let synth_ctx = voi_synth::Context::new(3, 256).unwrap();

        BasicPlugin {
            view: View::new(audio_cmd_tx),
            model: None,

            audio_cmd_rx,

            synth_ctx,
            // synth_info,
            // num_keys_down: 0,
            // voice_allocator: VoiceAllocator::new(synth_info.key_input),
        }
    }
}


impl BasicPlugin {
    fn process_midi_event(&mut self, evt: vst::event::MidiEvent) {
        let packet = evt.data;

        match packet[0] {
            0x80 ..= 0x8F => self.note_off(packet[1]),
            0x90 ..= 0x9F => {
                let key = packet[1];
                let velocity = packet[2];

                if velocity > 0 {
                    self.note_on(key, velocity);
                } else {
                    self.note_off(key);
                }
            }

            _ => {}
        }
    }

    fn note_on(&mut self, key: u8, velocity: u8) {
        if let Some(ref mut model) = self.model {
            model.voice_allocator.note_on(&mut self.synth_ctx, key, velocity as f32 / 127.0);
        }
    }

    fn note_off(&mut self, key: u8) {
        if let Some(ref mut model) = self.model {
            model.voice_allocator.note_off(&mut self.synth_ctx, key);
        }
    }

    fn process_audio_commands(&mut self) {
        while let Ok(audio_cmd) = self.audio_cmd_rx.try_recv() {
            match audio_cmd {
                AudioCommand::SetModel(src) => self.load_model(src),
            }
        }
    }

    fn load_model(&mut self, src: String) {
        if let Some(model) = self.model.take() {
            self.synth_ctx.remove_synth(model.synth_id);
        }

        match Model::from_string(&mut self.synth_ctx, src) {
            Ok(m) => {
                self.model = Some(m);
                log::info!("model loaded!");
            }
            Err(e) => {
                log::error!("failed to create model! {}", e);
            }
        }
    }
}

vst::plugin_main!(BasicPlugin);

