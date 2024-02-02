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
use crate::verbatim::suppressed_node;

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
        let FStringExpressionElement {
            expression,
            debug_text,
            conversion,
            format_spec,
            ..
        } = self.element;

        let comments = f.context().comments().clone();

        if let Some(debug_text) = debug_text {
            token("{").fmt(f)?;

            // If debug text is present in a f-string, we'll mark all of the comments
            // in this f-string as formatted.
            comments.mark_verbatim_node_comments_formatted(self.element.into());

            write!(
                f,
                [
                    text(&debug_text.leading, None),
                    // TODO: preserve the parentheses
                    suppressed_node(&**expression),
                    text(&debug_text.trailing, None),
                ]
            )?;

            // Even if debug text is present, any whitespace between the
            // conversion flag and the format spec doesn't need to be preserved.
            match conversion {
                ConversionFlag::Str => text("!s", None).fmt(f)?,
                ConversionFlag::Ascii => text("!a", None).fmt(f)?,
                ConversionFlag::Repr => text("!r", None).fmt(f)?,
                ConversionFlag::None => (),
            }

            if let Some(format_spec) = format_spec.as_deref() {
                write!(f, [token(":"), suppressed_node(format_spec)])?;
            }

            token("}").fmt(f)
        } else {
            let dangling_item_comments = comments.dangling(self.element);

            let item = format_with(|f| {
                let line_break_or_space = match expression.as_ref() {
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
                    Expr::Dict(_) | Expr::DictComp(_) | Expr::Set(_) | Expr::SetComp(_) => {
                        Some(soft_line_break_or_space())
                    }
                    _ => None,
                };

                write!(f, [line_break_or_space, expression.format()])?;

                // Conversion comes first, then the format spec.
                match conversion {
                    ConversionFlag::Str => text("!s", None).fmt(f)?,
                    ConversionFlag::Ascii => text("!a", None).fmt(f)?,
                    ConversionFlag::Repr => text("!r", None).fmt(f)?,
                    ConversionFlag::None => (),
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

                line_break_or_space.fmt(f)
            });

            parenthesized("{", &item, "}")
                .with_dangling_comments(dangling_item_comments)
                .fmt(f)
        }
    }
}
