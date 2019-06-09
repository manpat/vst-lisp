use crate::lisp::KeyInput;
use voi_synth::Context as SynthContext;
use voi_synth::ParameterID;

#[derive(Copy, Clone)]
struct ActiveKey {
    allocated_voice: Option<usize>,
    key: u8,
}

struct Voice {
    freq_param: Option<ParameterID>,
    vel_param: Option<ParameterID>,
    allocated_key: Option<u8>,
    // allocation time/signal volume
}

pub struct VoiceAllocator {
    active_keys: Vec<ActiveKey>,
    voices: Vec<Voice>,
}

impl VoiceAllocator {
    pub fn new(key_input_params: KeyInput) -> Self {
        let mut voices = Vec::new();

        match key_input_params {
            KeyInput::None => {}
            KeyInput::Mono{freq, vel} => {
                voices.push(Voice {
                    freq_param: freq,
                    vel_param: vel,
                    allocated_key: None,
                })
            }
        }

        VoiceAllocator {
            active_keys: Vec::new(),
            voices,
        }
    }

    pub fn note_on(&mut self, ctx: &mut SynthContext, key: u8, vel: f32) {
        let maybe_key_pos = self.active_keys.iter()
            .position(|k| k.key == key);

        // Key already on
        if let Some(active_key_pos) = maybe_key_pos {
            let mut active_key = self.active_keys[active_key_pos];

            // If the key hasn't been allocated a voice, try allocating one now
            if active_key.allocated_voice.is_none() {
                if let Some(voice_id) = self.try_start_voice(ctx, active_key.key, vel) {
                    active_key.allocated_voice = Some(voice_id); 
                }
            }

            // If the key already has an allocated voice or just got one, set the velocity
            if let Some(voice) = active_key.allocated_voice.map(|id| &self.voices[id]) {
                voice.set_vel(ctx, vel);
            }

            self.active_keys[active_key_pos] = active_key;

        } else {
            // This is a new key, try allocate a voice and push onto the queue
            let mut active_key = ActiveKey { key, allocated_voice: None };

            if let Some(voice_id) = self.try_start_voice(ctx, key, vel) {
                active_key.allocated_voice = Some(voice_id); 
            }

            self.active_keys.push(active_key);
        }
    }

    pub fn note_off(&mut self, ctx: &mut SynthContext, key: u8) {
        // Deactivate key
        let maybe_key_pos = self.active_keys.iter()
            .position(|k| k.key == key);

        if let Some(pos) = maybe_key_pos {
            self.active_keys.remove(pos);
        }

        // Deallocate voice
        let maybe_voice = self.voices.iter_mut()
            .find(|v| v.allocated_key == Some(key));

        if let Some(voice) = maybe_voice {
            voice.set_vel(ctx, 0.0);
            voice.allocated_key = None;
        }

        // TODO: reallocate voice if unallocated key in queue
    }

    fn try_start_voice(&mut self, ctx: &mut SynthContext, key: u8, vel: f32) -> Option<usize> {
        let freq = 440.0 * 2.0f32.powf((key as f32 - 64.0) / 12.0);

        // Try to find a free voice
        if let Some((id, voice)) = self.voices.iter_mut().enumerate().find(|(_, v)| v.allocated_key.is_none()) {
            voice.set_freq(ctx, freq);
            voice.set_vel(ctx, vel);
            voice.allocated_key = Some(key);
            return Some(id)
        }

        None
    }
}


impl Voice {
    fn set_freq(&self, ctx: &SynthContext, freq: f32) {
        if let Some(param) = self.freq_param {
            ctx.set_parameter(param, freq);
        }
    }

    fn set_vel(&self, ctx: &SynthContext, vel: f32) {
        if let Some(param) = self.vel_param {
            ctx.set_parameter(param, vel);
        }
    }
}