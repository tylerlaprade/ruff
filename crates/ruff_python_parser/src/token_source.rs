use crate::lexer::Spanned;
use crate::Tok;
use std::iter::FusedIterator;

#[derive(Clone, Debug)]
pub(crate) struct TokenSource {
    tokens: std::vec::IntoIter<Spanned>,
}

impl TokenSource {
    pub(crate) fn new(tokens: Vec<Spanned>) -> Self {
        Self {
            tokens: tokens.into_iter(),
        }
    }
}

impl Iterator for TokenSource {
    type Item = Spanned;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        loop {
            let next = self.tokens.next()?;

            if is_trivia(&next) {
                continue;
            }

            break Some(next);
        }
    }
}

impl FusedIterator for TokenSource {}

const fn is_trivia(result: &Spanned) -> bool {
    matches!(result, (Tok::Comment(_) | Tok::NonLogicalNewline, _))
}
