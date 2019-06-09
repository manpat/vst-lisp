use super::sexpression::{SExpression, ExpressionListExt};
use super::{LispResult, SynthInfo, KeyInput};
use voi_synth::failure::{format_err, bail, ensure};

use voi_synth::{
	Context as SynthContext,
	context::EvaluationContext as SynthEvaluationContext,
	node::Input as SynthInput,
	Synth,
	Buffer as SynthBuffer,
	NodeContainer,
	NodeID, synth::StoreID, ParameterID
};

use std::collections::HashMap;

macro_rules! ensure_args {
    ($func:expr, $list:ident == $count:expr) => {{
    	ensure!($list.len() == $count,
    		"'{}' function requires {} arguments, {} received",
    		$func, $count, $list.len())
    }};

    ($func:expr, $list:ident >= $count:expr) => {{
    	ensure!($list.len() >= $count,
    		"'{}' function requires at least {} arguments, {} received",
    		$func, $count, $list.len())
    }};
}


#[derive(Clone, Debug)]
enum EvalResult {
	Constant(f32),
	Array(Vec<f32>),
	SynthNode(SynthInput),
	// Function(),
}

impl EvalResult {
	fn expect_constant(self) -> LispResult<f32> {
		match self {
			EvalResult::Constant(f) => Ok(f),
			EvalResult::SynthNode(n) => bail!("Expected constant value, got node: {:?}", n),
			EvalResult::Array(n) => bail!("Expected constant value, got array: [{:?}]", n),
		}
	}
	fn expect_array(self) -> LispResult<Vec<f32>> {
		match self {
			EvalResult::Constant(f) => bail!("Expected array, got constant value: {:?}", f),
			EvalResult::SynthNode(n) => bail!("Expected array, got node: {:?}", n),
			EvalResult::Array(n) => Ok(n),
		}
	}
	fn to_input(self) -> LispResult<SynthInput> {
		match self {
			EvalResult::Constant(f) => Ok(f.into()),
			EvalResult::SynthNode(n) => Ok(n),
			EvalResult::Array(n) => bail!("Expected constant value or synth node, got array: [{:?}]", n),
		}
	}
	fn expect_node_id(self) -> LispResult<NodeID> {
		use self::SynthInput::*;

		match self {
			EvalResult::Constant(f) => bail!("Expected synth node, got constant: {}", f),
			EvalResult::Array(n) => bail!("Expected synth node, got array: [{:?}]", n),
			EvalResult::SynthNode(n) => match n {
				Literal(l) => bail!("Expected synth node, got Literal: {}", l),
				Node(n_id) => Ok(n_id),
				Store(s_id) => bail!("Expected synth node, got Store: {:?}", s_id),
				Parameter(p_id) => bail!("Expected synth node, got Parameter: {:?}", p_id),
			},
		}
	}
	fn expect_store_id(self) -> LispResult<StoreID> {
		use self::SynthInput::*;

		match self {
			EvalResult::Constant(f) => bail!("Expected synth store, got constant: {}", f),
			EvalResult::Array(n) => bail!("Expected synth store, got array: [{:?}]", n),
			EvalResult::SynthNode(n) => match n {
				Literal(l) => bail!("Expected synth store, got Literal: {}", l),
				Node(id) => bail!("Expected synth store, got Node: {:?}", id),
				Store(s_id) => Ok(s_id),
				Parameter(id) => bail!("Expected synth store, got Parameter: {:?}", id),
			},
		}
	}
}

impl Into<EvalResult> for f32 {
	fn into(self) -> EvalResult { EvalResult::Constant(self) }
}

impl Into<EvalResult> for SynthInput {
	fn into(self) -> EvalResult { EvalResult::SynthNode(self) }
}

impl Into<EvalResult> for NodeID {
	fn into(self) -> EvalResult { EvalResult::SynthNode(self.into()) }
}

impl Into<EvalResult> for StoreID {
	fn into(self) -> EvalResult { EvalResult::SynthNode(self.into()) }
}

impl Into<EvalResult> for ParameterID {
	fn into(self) -> EvalResult { EvalResult::SynthNode(self.into()) }
}


pub fn evaluate_top_level<'a>(ctx: &mut SynthContext, top_level: Vec<SExpression<'a>>) -> LispResult<(Synth, SynthInfo)> {
	let mut ctx = EvaluationContext::new(ctx);

	for sexpr in top_level {
		if let SExpression::List(mut list) = sexpr {
			if list.is_empty() {
				bail!("Tried to evaluate an empty list");
			}

			let func_name = list.remove(0).expect_ident()?;

			match func_name {
				"let" => {
					ensure_args!(func_name, list == 2);

					let ident = list.remove(0).expect_ident()?;
					let value = ctx.evaluate_sexpr(list.remove(0))?;

					ctx.let_bindings.insert(ident, value);
				}

				"gain" => {
					ensure_args!(func_name, list == 1);
					let gain = ctx.evaluate_sexpr(list.remove(0))?.expect_constant()?;
					ctx.synth.set_gain(gain);
				}

				"output" => {
					ensure_args!(func_name, list == 1);

					let node_id = ctx.evaluate_sexpr(list.remove(0))?.expect_node_id()?;
					ctx.synth.set_output(node_id);
				}

				"def-store" => {
					ensure_args!(func_name, list == 1);
					let ident = list.remove(0).expect_ident()?;
					let store = ctx.synth.new_value_store();
					ctx.let_bindings.insert(ident, store.into());
				}

				"store" => {
					ensure_args!(func_name, list == 2);
					let ident = ctx.evaluate_sexpr(list.remove(0))?;
					let value = ctx.evaluate_sexpr(list.remove(0))?;
					ctx.synth.new_store_write(ident.expect_store_id()?, value.to_input()?);
				}

				_ => {
					list.insert(0, SExpression::Identifier(func_name));
					ctx.execute_function(list)?;
				}
			}

		} else {
			bail!("Unexpected item at top level of synth definition: {:?}", sexpr);
		}
	}

	let info = SynthInfo{
		key_input: ctx.key_input,
	};

	Ok((ctx.synth, info))
}


struct EvaluationContext<'a> {
	synth_context: &'a mut SynthContext,
	synth: Synth,

	let_bindings: HashMap<&'a str, EvalResult>,
	key_input: KeyInput,
}


impl<'a> EvaluationContext<'a> {
	fn new(synth_context: &'a mut SynthContext) -> Self {
		EvaluationContext {
			synth_context,
			synth: Synth::new(),

			let_bindings: HashMap::new(),
			key_input: KeyInput::None,
		}
	}

	fn execute_function(&mut self, mut list: Vec<SExpression<'a>>) -> LispResult<EvalResult> {
		use std::cell::RefCell;

		if list.is_empty() {
			bail!("Tried to evaluate an empty list");
		}

		let func_name = list.remove(0).expect_ident()?;

		match func_name {
			"*" => {
				ensure_args!(func_name, list >= 2);

				let r_self = RefCell::new(self);

				// TODO: make better
				if list.is_constant() {
					let res = list.into_iter()
						.map(|expr| r_self.borrow_mut().evaluate_sexpr(expr)?.expect_constant())
						.fold(Ok(1.0), |a: LispResult<f32>, e| Ok(a? * e?));

					Ok(res?.into())

				} else {
					let a = r_self.borrow_mut().evaluate_sexpr(list.remove(0))?.to_input();
					// TODO: take advantage of associativity
					let res = list.into_iter()
						.map(|expr| r_self.borrow_mut().evaluate_sexpr(expr)?.to_input())
						.fold(a, |a, e| {
							Ok(r_self.borrow_mut().synth.new_multiply(a?, e?).into())
						});
					
					Ok(res?.into())
				}

			}

			"+" => {
				ensure_args!(func_name, list >= 2);

				let r_self = RefCell::new(self);

				// TODO: make better
				if list.is_constant() {
					let res = list.into_iter()
						.map(|expr| r_self.borrow_mut().evaluate_sexpr(expr)?.expect_constant())
						.fold(Ok(1.0), |a: LispResult<f32>, e| Ok(a? + e?));

					Ok(res?.into())

				} else {
					let a = r_self.borrow_mut().evaluate_sexpr(list.remove(0))?.to_input();
					// TODO: take advantage of associativity
					let res = list.into_iter()
						.map(|expr| r_self.borrow_mut().evaluate_sexpr(expr)?.to_input())
						.fold(a, |a, e| {
							Ok(r_self.borrow_mut().synth.new_add(a?, e?).into())
						});
					
					Ok(res?.into())
				}
			}

			"-" => {
				ensure_args!(func_name, list >= 2);

				let r_self = RefCell::new(self);

				// TODO: make better
				if list.is_constant() {
					let res = list.into_iter()
						.map(|expr| r_self.borrow_mut().evaluate_sexpr(expr)?.expect_constant())
						.fold(Ok(1.0), |a: LispResult<f32>, e| Ok(a? - e?));

					Ok(res?.into())

				} else {
					let a = r_self.borrow_mut().evaluate_sexpr(list.remove(0))?.to_input();
					let res = list.into_iter()
						.map(|expr| r_self.borrow_mut().evaluate_sexpr(expr)?.to_input())
						.fold(a, |a, e| {
							Ok(r_self.borrow_mut().synth.new_sub(a?, e?).into())
						});
					
					Ok(res?.into())
				}
			}

			"mix" => {
				ensure_args!(func_name, list == 3);
				let a = self.evaluate_sexpr(list.remove(0))?.to_input()?;
				let b = self.evaluate_sexpr(list.remove(0))?.to_input()?;
				let mix = self.evaluate_sexpr(list.remove(0))?.to_input()?;
				Ok(self.synth.new_mix(a, b, mix).into())
			}

			"sin" | "sine" => {
				ensure_args!(func_name, list == 1);
				let freq = self.evaluate_sexpr(list.remove(0))?.to_input()?;
				Ok(self.synth.new_sine(freq).into())
			}

			"tri" | "triangle" => {
				ensure_args!(func_name, list == 1);
				let freq = self.evaluate_sexpr(list.remove(0))?.to_input()?;
				Ok(self.synth.new_triangle(freq).into())
			}

			"sqr" | "square" => {
				ensure_args!(func_name, list == 1);
				let freq = self.evaluate_sexpr(list.remove(0))?.to_input()?;
				Ok(self.synth.new_square(freq).into())
			}

			"saw" | "sawtooth" => {
				ensure_args!(func_name, list == 1);
				let freq = self.evaluate_sexpr(list.remove(0))?.to_input()?;
				Ok(self.synth.new_saw(freq).into())
			}

			"lp" | "lowpass" => {
				ensure_args!(func_name, list == 2);
				let cutoff = self.evaluate_sexpr(list.remove(0))?.to_input()?;
				let input = self.evaluate_sexpr(list.remove(0))?.to_input()?;
				Ok(self.synth.new_lowpass(input, cutoff).into())
			}

			"hp" | "highpass" => {
				ensure_args!(func_name, list == 2);
				let cutoff = self.evaluate_sexpr(list.remove(0))?.to_input()?;
				let input = self.evaluate_sexpr(list.remove(0))?.to_input()?;
				Ok(self.synth.new_highpass(input, cutoff).into())
			}

			"ar" | "env-ar" => {
				ensure_args!(func_name, list == 3);
				let attack = self.evaluate_sexpr(list.remove(0))?.expect_constant()?;
				let release = self.evaluate_sexpr(list.remove(0))?.expect_constant()?;
				let gate = self.evaluate_sexpr(list.remove(0))?.to_input()?;
				Ok(self.synth.new_env_ar(attack, release, gate).into())
			}

			"adsr" | "env-adsr" => {
				ensure_args!(func_name, list == 5);
				let attack = self.evaluate_sexpr(list.remove(0))?.expect_constant()?;
				let decay = self.evaluate_sexpr(list.remove(0))?.expect_constant()?;
				let sustain = self.evaluate_sexpr(list.remove(0))?.expect_constant()?;
				let release = self.evaluate_sexpr(list.remove(0))?.expect_constant()?;
				let gate = self.evaluate_sexpr(list.remove(0))?.to_input()?;
				Ok(self.synth.new_env_adsr(attack, decay, sustain, release, gate).into())
			}

			"clamp" => {
				ensure_args!(func_name, list == 2);
				let input = self.evaluate_sexpr(list.remove(0))?.to_input()?;
				let lb = self.evaluate_sexpr(list.remove(0))?.to_input()?;
				let ub = self.evaluate_sexpr(list.remove(0))?.to_input()?;
				Ok(self.synth.new_clamp(input, lb, ub).into())
			}

			"sequencer" => {
				ensure_args!(func_name, list >= 2);
				let sequence = self.evaluate_sexpr(list.remove(0))?.expect_array()?;
				let advance = self.evaluate_sexpr(list.remove(0))?.to_input()?;
				let reset = if list.len() > 0 {
					self.evaluate_sexpr(list.remove(0))?.to_input()?
				} else { 1.0.into() };

				let buf = self.synth.new_buffer(sequence);
				Ok(self.synth.new_sequencer(buf, advance, reset).into())
			}

			"bake" => {
				ensure_args!(func_name, list >= 2);
				let sample_rate = self.synth_context.get_sample_rate();
				let samples = self.evaluate_sexpr(list.remove(0))?.expect_constant()? * sample_rate;
				let samples = samples as usize;

				ensure!(samples > 0, "You can't bake a synth to a zero length buffer");

				let (mut synth, _) = evaluate_top_level(self.synth_context, list)?;
				let mut eval_ctx = SynthEvaluationContext::new(sample_rate);
				let mut eval_buffer = SynthBuffer::new(samples);

				synth.evaluate_into_buffer(&mut eval_buffer, &mut eval_ctx);
				let buffer_id = self.synth.new_buffer(eval_buffer.data);

				Ok(self.synth.new_sampler(buffer_id, 0.0).into())
			}

			"key-freq" => {
				ensure_args!(func_name, list == 0);

				if let KeyInput::Mono{freq, ..} = &mut self.key_input {
					if let Some(param) = *freq {
						Ok(param.into())
					} else {
						let param = self.synth.new_parameter();
						*freq = Some(param);
						Ok(param.into())
					}

				} else {
					let param = self.synth.new_parameter();

					self.key_input = KeyInput::Mono {
						freq: Some(param),
						vel: None,
					};

					Ok(param.into())
				}
			}

			"key-vel" => {
				ensure_args!(func_name, list == 0);

				if let KeyInput::Mono{vel, ..} = &mut self.key_input {
					if let Some(param) = *vel {
						Ok(param.into())
					} else {
						let param = self.synth.new_parameter();
						*vel = Some(param);
						Ok(param.into())
					}

				} else {
					let param = self.synth.new_parameter();

					self.key_input = KeyInput::Mono {
						vel: Some(param),
						freq: None,
					};

					Ok(param.into())
				}
			}

			"polyphonic" => {
				ensure_args!(func_name, list >= 2);
				bail!("(polyphonic) Unimplemented")
			}

			_ => bail!("Unknown function: '{}'", func_name),
		}
	}

	fn evaluate_sexpr(&mut self, sexpr: SExpression<'a>) -> LispResult<EvalResult> {
		use self::SExpression::*;

		match sexpr {
			List(v) => self.execute_function(v),
			Number(n) => Ok(EvalResult::Constant(n)),

			Identifier(i) => {
				self.let_bindings.get(&i)
					.cloned()
					.ok_or_else(|| format_err!("Unknown identifier: '{}'", i))
			}

			Array(v) => {
				let mut rs = Vec::with_capacity(v.len());

				for sexpr in v {
					let result = self.evaluate_sexpr(sexpr)?;
					rs.push(result.expect_constant()?);
				}

				Ok(EvalResult::Array(rs))
			}
		}
	}
}