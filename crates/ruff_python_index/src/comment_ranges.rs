use std::fmt::Debug;

use ruff_python_ast::PySourceType;
use ruff_python_parser::lexer::lex;
use ruff_python_parser::{AsMode, Tok, Tokenized};
use ruff_python_trivia::CommentRanges;
use ruff_text_size::TextRange;

#[derive(Debug, Clone, Default)]
pub struct CommentRangesBuilder {
    ranges: Vec<TextRange>,
}

impl CommentRangesBuilder {
    pub fn visit_token(&mut self, token: &Tok, range: TextRange) {
        if token.is_comment() {
            self.ranges.push(range);
        }
    }

    pub fn finish(self) -> CommentRanges {
        CommentRanges::new(self.ranges)
    }
}

/// Helper method to lex and extract comment ranges
pub fn tokens_and_ranges(source: &str, source_type: PySourceType) -> (Tokenized, CommentRanges) {
    let mut tokens = Vec::new();
    let mut comment_ranges = CommentRangesBuilder::default();
    let mut lexer = lex(source, source_type.as_mode());

    for result in lexer.by_ref() {
        comment_ranges.visit_token(&result.0, result.1);

        tokens.push(result);
    }

    let comment_ranges = comment_ranges.finish();
    (Tokenized::new(tokens, lexer.into_errors()), comment_ranges)
}
