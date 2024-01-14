use std::borrow::Cow;

use ruff_formatter::write;
use ruff_python_ast::{
    ConversionFlag, Expr, FStringElement, FStringExpressionElement, FStringLiteralElement,
};
use ruff_text_size::Ranged;

use crate::expression::parentheses::parenthesized;
use crate::prelude::*;
use crate::preview::is_hex_codes_in_unicode_sequences_enabled;
use crate::string::normalize_string;

use super::f_string::FStringContext;

/// Formats an f-string element which is either a literal or a formatted expression.
///
/// This delegates the actual formatting to the appropriate formatter.
pub(crate) struct FormatFStringElement<'a> {
    element: &'a FStringElement,
    context: FStringContext,
}

impl<'a> FormatFStringElement<'a> {
    pub(crate) fn new(element: &'a FStringElement, context: FStringContext) -> Self {
        Self { element, context }
    }
}

impl Format<PyFormatContext<'_>> for FormatFStringElement<'_> {
    fn fmt(&self, f: &mut PyFormatter) -> FormatResult<()> {
        match self.element {
            FStringElement::Literal(string_literal) => {
                FormatFStringLiteralElement::new(string_literal, self.context).fmt(f)
            }
            FStringElement::Expression(expression) => {
                FormatFStringExpressionElement::new(expression, self.context).fmt(f)
            }
        }
    }
}

pub(crate) struct FormatFStringLiteralElement<'a> {
    element: &'a FStringLiteralElement,
    context: FStringContext,
}

impl<'a> FormatFStringLiteralElement<'a> {
    pub(crate) fn new(element: &'a FStringLiteralElement, context: FStringContext) -> Self {
        Self { element, context }
    }
}

impl Format<PyFormatContext<'_>> for FormatFStringLiteralElement<'_> {
    fn fmt(&self, f: &mut PyFormatter) -> FormatResult<()> {
        let literal_content = f.context().locator().slice(self.element.range());
        let normalized = normalize_string(
            literal_content,
            self.context.quotes(),
            self.context.prefix(),
            is_hex_codes_in_unicode_sequences_enabled(f.context()),
        );
        match &normalized {
            Cow::Borrowed(_) => source_text_slice(self.element.range()).fmt(f),
            Cow::Owned(normalized) => text(normalized, Some(self.element.start())).fmt(f),
        }
    }
}

pub(crate) struct FormatFStringExpressionElement<'a> {
    element: &'a FStringExpressionElement,
    context: FStringContext,
}

impl<'a> FormatFStringExpressionElement<'a> {
    pub(crate) fn new(element: &'a FStringExpressionElement, context: FStringContext) -> Self {
        Self { element, context }
    }
}

impl Format<PyFormatContext<'_>> for FormatFStringExpressionElement<'_> {
    fn fmt(&self, f: &mut PyFormatter) -> FormatResult<()> {
        // Notes for implementing f-string formatting for 3.12 or later:
        //
        // - Currently, if there are comments in the f-string, we abort formatting
        //   and fall back to the string normalization. If comments are supported,
        //   then they need to be handled separately when debug text is present.
        //   This is because the comments are present in the debug text in the raw
        //   form. One solution would be to mark all comments as formatted and
        //   add the debug text as it is.

        let FStringExpressionElement {
            expression,
            debug_text,
            conversion,
            format_spec,
            ..
        } = self.element;

        // If an expression starts with a `{`, we need to add a space before the
        // curly brace to avoid turning it into a literal curly with `{{`.
        //
        // For example,
        // ```python
        // f"{ {'x': 1, 'y': 2} }"
        // #  ^                ^
        // ```
        //
        // We need to preserve the space highlighted by `^`.
        let line_break_or_space = if debug_text.is_some() {
            None
        } else {
            match expression.as_ref() {
                Expr::Dict(_) | Expr::DictComp(_) | Expr::Set(_) | Expr::SetComp(_) => {
                    // This should either be a line break or a space to avoid adding
                    // a leading space to the expression in case the expression
                    // breaks over multiple lines.
                    //
                    // This is especially important when there's a comment present.
                    // For example,
                    //
                    // ```python
                    // f"something {
                    //     # comment
                    //     {'a': 1, 'b': 2}
                    // } ending"
                    // ```
                    //
                    // If we would unconditionally add a space, there would be a
                    // trailing space before the comment.
                    Some(soft_line_break_or_space())
                }
                _ => None,
            }
        };

        let conversion_text = match conversion {
            ConversionFlag::Str => Some(text("!s", None)),
            ConversionFlag::Ascii => Some(text("!a", None)),
            ConversionFlag::Repr => Some(text("!r", None)),
            ConversionFlag::None => None,
        };

        let inner =
            &format_with(|f| {
                if let Some(debug_text) = debug_text {
                    text(&debug_text.leading, None).fmt(f)?;
                }

                write!(
                    f,
                    [
                        line_break_or_space,
                        expression.format(),
                        // The extra whitespace isn't strictly required for the
                        // ending curly brace but it's here for symmetry.
                        //
                        // For example, the following is valid:
                        // ```python
                        // f"{ {'a': 1}}"
                        // ```
                        //
                        // But, the following looks better:
                        // ```python
                        // f"{ {'a': 1} }"
                        // ```
                        line_break_or_space,
                        conversion_text,
                    ]
                )?;

                if let Some(debug_text) = debug_text {
                    text(&debug_text.trailing, None).fmt(f)?;
                }

                if let Some(format_spec) = format_spec.as_deref() {
                    let elements =
                        format_with(|f| {
                            f.join()
                                .entries(format_spec.elements.iter().map(|element| {
                                    FormatFStringElement::new(element, self.context)
                                }))
                                .finish()
                        });
                    write!(f, [token(":"), elements])?;
                }

                Ok(())
            });

        if self.context.quotes().is_triple() {
            parenthesized("{", inner, "}").fmt(f)
        } else {
            write!(f, [token("{"), inner, token("}")])
        }
    }
}
