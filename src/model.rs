use voi_synth::{Synth, SynthID, ParameterID, Context as SynthContext};

use crate::VstResult;
use crate::voice_allocator::VoiceAllocator;

pub struct Model {
    pub synth_id: SynthID,
    pub voice_allocator: VoiceAllocator,

    pub source: String,
}

impl Model {
    pub fn from_string(synth_ctx: &mut SynthContext, src: String) -> VstResult<Model> {
        let (synth_id, synth_info) = crate::lisp::create_synth(synth_ctx, &src)?;

        Ok(Model{
            synth_id,
            voice_allocator: VoiceAllocator::new(synth_info.key_input),

            source: src,
        })
    }
}