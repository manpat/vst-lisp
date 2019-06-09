mod sexpression;
mod parser;
mod evaluation;

use voi_synth::{
	Context as SynthContext,
	ParameterID, SynthID
};

use crate::VstResult as LispResult;

pub struct SynthInfo {
	pub key_input: KeyInput,
}

pub enum KeyInput {
	None,
	Mono {
		// TODO: voice stealing mode
		freq: Option<ParameterID>,
		vel: Option<ParameterID>,
	},
}

pub fn create_synth(ctx: &mut SynthContext, input: &str) -> LispResult<(SynthID, SynthInfo)> {
	use std::iter::once;

	let comment_free_input = input.lines()
		.map(|l| l.split(';').next().unwrap())
		.flat_map(|l| l.chars().chain(once('\n')) )
		.collect::<String>();

	let top_level_exprs = parser::ExprReader::new(&comment_free_input).parse_toplevel()?;

	let (synth, info) = evaluation::evaluate_top_level(ctx, top_level_exprs)?;

	log::info!("{:?}", synth);

	ctx.push_synth(synth)
		.map(|id| (id, info))
}