use super::LispResult;
use voi_synth::failure::{format_err, bail};


#[derive(Clone, Debug)]
pub enum SExpression<'a> {
	Identifier(&'a str),
	Number(f32),
	List(Vec<SExpression<'a>>),
	Array(Vec<SExpression<'a>>),
}

use self::SExpression::*;

impl<'a> SExpression<'a> {
	pub fn expect_ident(self) -> LispResult<&'a str> {
		match self {
			Identifier(s) => Ok(s),
			Number(x) => bail!("Expected identifier, got number: {}", x),
			List(v) => bail!("Expected identifier, got list: ({:?})", v),
			Array(v) => bail!("Expected identifier, got array: ({:?})", v),
		}
	}
}



pub trait ExpressionListExt {
	fn is_constant(&self) -> bool;
}

impl<'a> ExpressionListExt for Vec<SExpression<'a>> {
	fn is_constant(&self) -> bool {
		self.iter().all(|sexpr| match *sexpr {
			SExpression::Number(_) => true,
			// SExpression::Identifier(_) => true, // TODO
			_ => false,
		})
	}
}
