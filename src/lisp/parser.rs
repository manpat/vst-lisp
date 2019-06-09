
use super::LispResult;
use super::sexpression::SExpression;

use voi_synth::failure::{format_err, bail};

use self::SExpression::*;

#[derive(Copy, Clone, Debug)]
pub struct ExprReader<'a> {
	input: &'a str,
}

impl<'a> ExprReader<'a> {
	pub fn new(input: &str) -> ExprReader {
		ExprReader {input}
	}

	pub fn is_empty(&self) -> bool { self.input.is_empty() }

	pub fn peek(&self) -> LispResult<char> {
		self.input.chars()
			.next()
			.ok_or_else(|| format_err!("Hit end of input"))
	}

	pub fn expect(&mut self, c: char) -> LispResult<()> {
		self.skip_whitespace();
		let next = self.peek()?;

		if next != c {
			bail!("Unexpected character '{}', expected '{}'", next, c)
		}

		self.input = &self.input[next.len_utf8()..];
		Ok(())
	}

	pub fn skip_whitespace(&mut self) {
		self.input = self.input.trim_start();
	}

	pub fn parse_toplevel(&mut self) -> LispResult<Vec<SExpression<'a>>> {
		let mut top_level_exprs = Vec::new();

		self.skip_whitespace();

		while !self.is_empty() {
			top_level_exprs.push(self.parse_sexpression()?);
			self.skip_whitespace();
		}

		Ok(top_level_exprs)
	}

	pub fn parse_sexpression(&mut self) -> LispResult<SExpression<'a>> {
		match self.peek()? {
			'(' => {
				let list = self.parse_list('(', ')')?;
				Ok( List(list) )
			}

			'[' => {
				let list = self.parse_list('[', ']')?;
				Ok( Array(list) )
			}

			_ => {
				let word = self.parse_word()?;

				if let Ok(f) = word.parse() {
					Ok( Number(f) )
				} else {
					Ok( Identifier(word) )
				}
			}
		}
	}

	pub fn parse_word(&mut self) -> LispResult<&'a str> {
		self.skip_whitespace();

		let word_end = self.input
			.find(char::is_whitespace)
			.unwrap_or(self.input.len());

		let (word, rest) = self.input.split_at(word_end);
		self.input = rest;
		Ok(word)
	}

	pub fn parse_list(&mut self, open: char, close: char) -> LispResult<Vec<SExpression<'a>>> {
		let mut list_parser = self.list_parser(open, close)?;
		let mut ret = Vec::new();

		list_parser.skip_whitespace();

		while !list_parser.is_empty() {
			ret.push(list_parser.parse_sexpression()?);
			list_parser.skip_whitespace();
		}
		
		Ok(ret)
	}

	fn list_parser(&mut self, open: char, close: char) -> LispResult<ExprReader<'a>> {
		self.expect(open)?;

		let end = self.input
			.char_indices()
			.scan(1, |level, (pos, c)| {
				match c {
					c if (c == open) => { *level += 1 }
					c if (c == close) => { *level -= 1 }
					_ => {}
				}

				Some((*level, pos))
			})
			.find(|(l, _)| *l == 0);

		if let Some((_, pos)) = end {
			let (list_str, rest) = self.input.split_at(pos);

			self.input = rest;
			self.expect(close)?;

			Ok(ExprReader::new(list_str))
		} else {
			bail!("Couldn't find end of the list");
		}
	}
}
