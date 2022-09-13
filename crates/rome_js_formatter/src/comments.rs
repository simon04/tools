use crate::prelude::*;
use crate::utils::JsAnyConditional;
use rome_formatter::{write, CommentPlacement, CommentPosition, Comments, DecoratedComment};
use rome_formatter::{CommentKind, CommentStyle, SourceComment};
use rome_js_syntax::suppression::{parse_suppression_comment, SuppressionCategory};
use rome_js_syntax::{
    JsAnyClass, JsAnyName, JsAnyRoot, JsAnyStatement, JsArrayHole, JsArrowFunctionExpression,
    JsBlockStatement, JsCallArguments, JsCatchClause, JsEmptyStatement, JsFinallyClause,
    JsFormalParameter, JsFunctionBody, JsIdentifierExpression, JsIfStatement, JsLanguage,
    JsSyntaxKind, JsSyntaxNode, JsVariableDeclarator, JsWhileStatement, TsInterfaceDeclaration,
};
use rome_rowan::{AstNode, SyntaxTriviaPieceComments, TextLen};

pub type JsComments = Comments<JsLanguage>;

#[derive(Default)]
pub struct FormatJsLeadingComment;

impl FormatRule<SourceComment<JsLanguage>> for FormatJsLeadingComment {
    type Context = JsFormatContext;

    fn fmt(
        &self,
        comment: &SourceComment<JsLanguage>,
        f: &mut Formatter<Self::Context>,
    ) -> FormatResult<()> {
        if is_doc_comment(comment.piece()) {
            let mut source_offset = comment.piece().text_range().start();

            let mut lines = comment.piece().text().lines();

            // SAFETY: Safe, `is_doc_comment` only returns `true` for multiline comments
            let first_line = lines.next().unwrap();
            write!(f, [dynamic_text(first_line.trim_end(), source_offset)])?;

            source_offset += first_line.text_len();

            // Indent the remaining lines by one space so that all `*` are aligned.
            write!(
                f,
                [align(
                    1,
                    &format_once(|f| {
                        for line in lines {
                            write!(
                                f,
                                [hard_line_break(), dynamic_text(line.trim(), source_offset)]
                            )?;

                            source_offset += line.text_len();
                        }

                        Ok(())
                    })
                )]
            )
        } else {
            write!(f, [comment.piece().as_piece()])
        }
    }
}

/// Returns `true` if `comment` is a multi line block comment:
///
/// # Examples
///
/// ```
/// # use rome_js_parser::parse_module;
/// # use rome_js_syntax::JsLanguage;
/// # use rome_rowan::{Direction, SyntaxTriviaPieceComments};
///  use rome_js_formatter::comments::is_doc_comment;
///
/// # fn parse_comment(source: &str) -> SyntaxTriviaPieceComments<JsLanguage> {
/// #     let root = parse_module(source, 0).tree();
/// #     root
/// #        .eof_token()
/// #        .expect("Root to have an EOF token")
/// #        .leading_trivia()
/// #        .pieces()
/// #        .filter_map(|piece| piece.as_comments())
/// #        .next()
/// #        .expect("Source to contain a comment.")
/// # }
///
/// assert!(is_doc_comment(&parse_comment(r#"
///     /**
///      * Multiline doc comment
///      */
/// "#)));
///
/// assert!(is_doc_comment(&parse_comment(r#"
///     /*
///      * Single star
///      */
/// "#)));
///
///
/// // Non doc-comments
/// assert!(!is_doc_comment(&parse_comment(r#"/** has no line break */"#)));
///
/// assert!(!is_doc_comment(&parse_comment(r#"
/// /*
///  *
///  this line doesn't start with a star
///  */
/// "#)));
/// ```
pub fn is_doc_comment(comment: &SyntaxTriviaPieceComments<JsLanguage>) -> bool {
    if !comment.has_newline() {
        return false;
    }

    let text = comment.text();

    text.lines().enumerate().all(|(index, line)| {
        if index == 0 {
            line.starts_with("/*")
        } else {
            line.trim_start().starts_with('*')
        }
    })
}

#[derive(Eq, PartialEq, Copy, Clone, Debug, Default)]
pub struct JsCommentStyle;

impl CommentStyle for JsCommentStyle {
    type Language = JsLanguage;

    fn is_suppression(text: &str) -> bool {
        parse_suppression_comment(text)
            .flat_map(|suppression| suppression.categories)
            .any(|(category, _)| category == SuppressionCategory::Format)
    }

    fn get_comment_kind(comment: &SyntaxTriviaPieceComments<JsLanguage>) -> CommentKind {
        if comment.text().starts_with("/*") {
            if comment.has_newline() {
                CommentKind::Block
            } else {
                CommentKind::InlineBlock
            }
        } else {
            CommentKind::Line
        }
    }

    fn place_comment(
        &self,
        comment: DecoratedComment<Self::Language>,
    ) -> CommentPlacement<Self::Language> {
        match comment.position() {
            CommentPosition::EndOfLine => handle_typecast_comment(comment)
                .or_else(handle_function_declaration_comment)
                .or_else(handle_conditional_comment)
                .or_else(handle_if_statement_comment)
                .or_else(handle_while_comment)
                .or_else(handle_try_comment)
                .or_else(handle_class_comment)
                .or_else(handle_method_comment)
                .or_else(handle_for_comment)
                .or_else(handle_root_comments)
                .or_else(handle_array_hole_comment)
                .or_else(handle_variable_declarator_comment)
                .or_else(handle_parameter_comment)
                .or_else(handle_labelled_statement_comment)
                .or_else(handle_call_expression_comment)
                .or_else(handle_continue_break_comment)
                .or_else(handle_mapped_type_comment)
                .or_else(handle_switch_default_case_comment),
            CommentPosition::OwnLine => handle_member_expression_comment(comment)
                .or_else(handle_function_declaration_comment)
                .or_else(handle_if_statement_comment)
                .or_else(handle_while_comment)
                .or_else(handle_try_comment)
                .or_else(handle_class_comment)
                .or_else(handle_method_comment)
                .or_else(handle_for_comment)
                .or_else(handle_root_comments)
                .or_else(handle_parameter_comment)
                .or_else(handle_array_hole_comment)
                .or_else(handle_labelled_statement_comment)
                .or_else(handle_call_expression_comment)
                .or_else(handle_mapped_type_comment)
                .or_else(handle_continue_break_comment),
            CommentPosition::SameLine => handle_if_statement_comment(comment)
                .or_else(handle_while_comment)
                .or_else(handle_for_comment)
                .or_else(handle_root_comments)
                .or_else(handle_after_arrow_param_comment)
                .or_else(handle_array_hole_comment)
                .or_else(handle_call_expression_comment)
                .or_else(handle_continue_break_comment),
        }
    }
}

/// Force end of line type cast comments to remain leading comments of the next node, if any
fn handle_typecast_comment(comment: DecoratedComment<JsLanguage>) -> CommentPlacement<JsLanguage> {
    match comment.following_node() {
        Some(following_node) if is_type_comment(comment.piece()) => CommentPlacement::Leading {
            node: following_node.clone(),
            comment,
        },
        _ => CommentPlacement::Default(comment),
    }
}

fn handle_after_arrow_param_comment(
    comment: DecoratedComment<JsLanguage>,
) -> CommentPlacement<JsLanguage> {
    let is_next_arrow = comment.following_token().kind() == JsSyntaxKind::FAT_ARROW;

    // Makes comments after the `(` and `=>` dangling comments
    // ```javascript
    // () /* comment */ => true
    // ```
    if JsArrowFunctionExpression::can_cast(comment.enclosing_node().kind()) && is_next_arrow {
        CommentPlacement::Dangling {
            node: comment.enclosing_node().clone(),
            comment,
        }
    } else {
        CommentPlacement::Default(comment)
    }
}

/// Handles array hole comments. Array holes have no token so all comments
/// become trailing comments by default. Override it that all comments are leading comments.
fn handle_array_hole_comment(
    comment: DecoratedComment<JsLanguage>,
) -> CommentPlacement<JsLanguage> {
    if let Some(array_hole) = comment.preceding_node().and_then(JsArrayHole::cast_ref) {
        CommentPlacement::Leading {
            node: array_hole.into_syntax(),
            comment,
        }
    } else {
        CommentPlacement::Default(comment)
    }
}

fn handle_call_expression_comment(
    comment: DecoratedComment<JsLanguage>,
) -> CommentPlacement<JsLanguage> {
    // Make comments between the callee and the arguments leading comments of the first argument.
    // ```javascript
    // a /* comment */ (call)
    // ```
    if let Some(arguments) = comment.following_node().and_then(JsCallArguments::cast_ref) {
        return if let Some(Ok(first)) = arguments.args().first() {
            CommentPlacement::Leading {
                node: first.into_syntax(),
                comment,
            }
        } else {
            CommentPlacement::Dangling {
                node: arguments.into_syntax(),
                comment,
            }
        };
    }

    CommentPlacement::Default(comment)
}

fn handle_continue_break_comment(
    comment: DecoratedComment<JsLanguage>,
) -> CommentPlacement<JsLanguage> {
    let enclosing = comment.enclosing_node();

    // Make comments between the `continue` and label token trailing comments
    // ```javascript
    // continue /* comment */ a;
    // ```
    // This differs from Prettier because other ASTs use an identifier for the label whereas Rome uses
    // a token.
    match enclosing.kind() {
        JsSyntaxKind::JS_CONTINUE_STATEMENT | JsSyntaxKind::JS_BREAK_STATEMENT => {
            match enclosing.parent() {
                // Make it the trailing of the parent if this is a single-statement body
                // to prevent that the comment becomes a trailing comment of the parent when re-formatting
                // ```javascript
                // for (;;) continue /* comment */;
                // ```
                Some(parent)
                    if matches!(
                        parent.kind(),
                        JsSyntaxKind::JS_FOR_STATEMENT
                            | JsSyntaxKind::JS_FOR_OF_STATEMENT
                            | JsSyntaxKind::JS_FOR_IN_STATEMENT
                            | JsSyntaxKind::JS_WHILE_STATEMENT
                            | JsSyntaxKind::JS_DO_WHILE_STATEMENT
                            | JsSyntaxKind::JS_IF_STATEMENT
                            | JsSyntaxKind::JS_WITH_STATEMENT
                            | JsSyntaxKind::JS_LABELED_STATEMENT
                    ) =>
                {
                    CommentPlacement::Trailing {
                        node: parent,
                        comment,
                    }
                }
                _ => CommentPlacement::Trailing {
                    node: enclosing.clone(),
                    comment,
                },
            }
        }
        _ => CommentPlacement::Default(comment),
    }
}

/// Moves line comments after the `default` keyword into the block statement:
///
/// ```javascript
/// switch (x) {
///	    default: // comment
///     {
/// 		break;
///     }
/// ```
///
/// All other same line comments become `Dangling` comments that are handled inside of the default case
/// formatting
fn handle_switch_default_case_comment(
    comment: DecoratedComment<JsLanguage>,
) -> CommentPlacement<JsLanguage> {
    match (comment.enclosing_node().kind(), comment.following_node()) {
        (JsSyntaxKind::JS_DEFAULT_CLAUSE, Some(following)) => {
            match JsBlockStatement::cast_ref(following) {
                Some(block) if comment.kind().is_line() => {
                    place_block_statement_comment(block, comment)
                }
                _ => CommentPlacement::Dangling {
                    node: comment.enclosing_node().clone(),
                    comment,
                },
            }
        }
        _ => CommentPlacement::Default(comment),
    }
}

fn handle_labelled_statement_comment(
    comment: DecoratedComment<JsLanguage>,
) -> CommentPlacement<JsLanguage> {
    match comment.enclosing_node().kind() {
        JsSyntaxKind::JS_LABELED_STATEMENT => CommentPlacement::Leading {
            node: comment.enclosing_node().clone(),
            comment,
        },
        _ => CommentPlacement::Default(comment),
    }
}

fn handle_class_comment(comment: DecoratedComment<JsLanguage>) -> CommentPlacement<JsLanguage> {
    // Make comments following after the `extends` or `implements` keyword trailing comments
    // of the preceding extends, type parameters, or id.
    // ```javascript
    // class a9 extends
    //   /* comment */
    // b {
    //   constructor() {}
    // }
    // ```
    if matches!(
        comment.enclosing_node().kind(),
        JsSyntaxKind::JS_EXTENDS_CLAUSE
            | JsSyntaxKind::TS_IMPLEMENTS_CLAUSE
            | JsSyntaxKind::TS_EXTENDS_CLAUSE
    ) && comment.preceding_node().is_none()
    {
        if let Some(sibling) = comment.enclosing_node().prev_sibling() {
            return CommentPlacement::Trailing {
                node: sibling,
                comment,
            };
        }
    }

    let first_member = if let Some(class) = JsAnyClass::cast_ref(comment.enclosing_node()) {
        class.members().first().map(AstNode::into_syntax)
    } else if let Some(interface) = TsInterfaceDeclaration::cast_ref(comment.enclosing_node()) {
        interface.members().first().map(AstNode::into_syntax)
    } else {
        return CommentPlacement::Default(comment);
    };

    if let Some(following) = comment.following_node() {
        // Make comments preceding the first member leading comments of the member
        // ```javascript
        // class Test { /* comment */
        //      prop;
        // }
        // ```
        if let Some(member) = first_member {
            if following == &member {
                return CommentPlacement::Leading {
                    node: member,
                    comment,
                };
            }
        }

        if let Some(preceding) = comment.preceding_node() {
            // Make all comments between the id/type parameters and the extends clause trailing comments
            // of the id/type parameters
            //
            // ```javascript
            // class Test
            //
            // /* comment */ extends B {}
            // ```
            if matches!(
                following.kind(),
                JsSyntaxKind::JS_EXTENDS_CLAUSE
                    | JsSyntaxKind::TS_IMPLEMENTS_CLAUSE
                    | JsSyntaxKind::TS_EXTENDS_CLAUSE
            ) {
                return CommentPlacement::Trailing {
                    node: preceding.clone(),
                    comment,
                };
            }
        }
    } else if first_member.is_none() {
        // Handle the case where there are no members, attach the comments as dangling comments.
        // ```javascript
        // class Test // comment
        // {
        // }
        // ```
        return CommentPlacement::Dangling {
            node: comment.enclosing_node().clone(),
            comment,
        };
    }

    CommentPlacement::Default(comment)
}

fn handle_method_comment(comment: DecoratedComment<JsLanguage>) -> CommentPlacement<JsLanguage> {
    let enclosing = comment.enclosing_node();

    let is_method = matches!(
        enclosing.kind(),
        JsSyntaxKind::JS_METHOD_CLASS_MEMBER
            | JsSyntaxKind::JS_METHOD_OBJECT_MEMBER
            | JsSyntaxKind::JS_SETTER_CLASS_MEMBER
            | JsSyntaxKind::JS_GETTER_CLASS_MEMBER
            | JsSyntaxKind::JS_SETTER_OBJECT_MEMBER
            | JsSyntaxKind::JS_GETTER_OBJECT_MEMBER
            | JsSyntaxKind::JS_CONSTRUCTOR_CLASS_MEMBER
    );

    if !is_method {
        return CommentPlacement::Default(comment);
    }

    // Move end of line and own line comments before the method body into the function
    // ```javascript
    // class Test {
    //   method() /* test */
    //  {}
    // }
    // ```
    if let Some(following) = comment.following_node() {
        if let Some(body) = JsFunctionBody::cast_ref(following) {
            if let Some(directive) = body.directives().first() {
                return CommentPlacement::Leading {
                    node: directive.into_syntax(),
                    comment,
                };
            }

            let first_non_empty = body
                .statements()
                .iter()
                .find(|statement| !matches!(statement, JsAnyStatement::JsEmptyStatement(_)));

            return match first_non_empty {
                None => CommentPlacement::Dangling {
                    node: body.into_syntax(),
                    comment,
                },
                Some(statement) => CommentPlacement::Leading {
                    node: statement.into_syntax(),
                    comment,
                },
            };
        }
    }

    CommentPlacement::Default(comment)
}

/// Handle a all comments document.
/// See `blank.js`
fn handle_root_comments(comment: DecoratedComment<JsLanguage>) -> CommentPlacement<JsLanguage> {
    if let Some(root) = JsAnyRoot::cast_ref(comment.enclosing_node()) {
        let is_blank = match &root {
            JsAnyRoot::JsExpressionSnipped(_) => false,
            JsAnyRoot::JsModule(module) => {
                module.directives().is_empty() && module.items().is_empty()
            }
            JsAnyRoot::JsScript(script) => {
                script.directives().is_empty() && script.statements().is_empty()
            }
        };

        if is_blank {
            return CommentPlacement::Leading {
                node: root.into_syntax(),
                comment,
            };
        }
    }

    CommentPlacement::Default(comment)
}

fn handle_member_expression_comment(
    comment: DecoratedComment<JsLanguage>,
) -> CommentPlacement<JsLanguage> {
    let following = match comment.following_node() {
        Some(following)
            if matches!(
                comment.enclosing_node().kind(),
                JsSyntaxKind::JS_STATIC_MEMBER_EXPRESSION
                    | JsSyntaxKind::JS_COMPUTED_MEMBER_EXPRESSION
            ) =>
        {
            following
        }
        _ => return CommentPlacement::Default(comment),
    };

    // ```javascript
    // a
    // /* comment */.b
    // a
    // /* comment */[b]
    // ```
    if JsAnyName::can_cast(following.kind()) || JsIdentifierExpression::can_cast(following.kind()) {
        CommentPlacement::Leading {
            node: comment.enclosing_node().clone(),
            comment,
        }
    } else {
        CommentPlacement::Default(comment)
    }
}

fn handle_function_declaration_comment(
    comment: DecoratedComment<JsLanguage>,
) -> CommentPlacement<JsLanguage> {
    let is_function_declaration = matches!(
        comment.enclosing_node().kind(),
        JsSyntaxKind::JS_FUNCTION_DECLARATION
            | JsSyntaxKind::JS_FUNCTION_EXPORT_DEFAULT_DECLARATION
    );

    let following = match comment.following_node() {
        Some(following) if is_function_declaration => following,
        _ => return CommentPlacement::Default(comment),
    };

    // Make comments between the `)` token and the function body leading comments
    // of the first non empty statement or dangling comments of the body.
    // ```javascript
    // function test() /* comment */ {
    //  console.log("Hy");
    // }
    // ```
    if let Some(body) = JsFunctionBody::cast_ref(following) {
        match body
            .statements()
            .iter()
            .find(|statement| !matches!(statement, JsAnyStatement::JsEmptyStatement(_)))
        {
            Some(first) => CommentPlacement::Leading {
                node: first.into_syntax(),
                comment,
            },
            None => CommentPlacement::Dangling {
                node: body.into_syntax(),
                comment,
            },
        }
    } else {
        CommentPlacement::Default(comment)
    }
}

fn handle_conditional_comment(
    comment: DecoratedComment<JsLanguage>,
) -> CommentPlacement<JsLanguage> {
    let enclosing = comment.enclosing_node();

    let (conditional, following) = match (
        JsAnyConditional::cast_ref(enclosing),
        comment.following_node(),
    ) {
        (Some(conditional), Some(following)) => (conditional, following),
        _ => {
            return CommentPlacement::Default(comment);
        }
    };

    // Make end of line comments that come after the operator leading comments of the consequent / alternate.
    // ```javascript
    // a
    //   // becomes leading of consequent
    //   ? { x: 5 } :
    //   {};
    //
    // a
    //   ? { x: 5 }
    //   : // becomes leading of alternate
    // 	{};
    //
    // a // remains trailing, because it directly follows the node
    //   ? { x: 5 }
    //   : {};
    // ```
    let token = comment.piece().as_piece().token();
    let is_after_operator = conditional.colon_token().as_ref() == Ok(&token)
        || conditional.question_mark_token().as_ref() == Ok(&token);

    if is_after_operator {
        return CommentPlacement::Leading {
            node: following.clone(),
            comment,
        };
    }

    CommentPlacement::Default(comment)
}

fn handle_if_statement_comment(
    comment: DecoratedComment<JsLanguage>,
) -> CommentPlacement<JsLanguage> {
    fn handle_else_clause(
        comment: DecoratedComment<JsLanguage>,
        consequent: JsSyntaxNode,
        if_statement: JsSyntaxNode,
    ) -> CommentPlacement<JsLanguage> {
        // Make all comments trailing comments of the `consequent` if the `consequent` is a `JsBlockStatement`
        // ```javascript
        // if (test) {
        //
        // } /* comment */ else if (b) {
        //     test
        // }
        // /* comment */ else if(c) {
        //
        // } /*comment */ else {
        //
        // }
        // ```
        if consequent.kind() == JsSyntaxKind::JS_BLOCK_STATEMENT {
            return CommentPlacement::Trailing {
                node: consequent,
                comment,
            };
        }

        // Handle end of line comments that aren't stretching over multiple lines.
        // Make them dangling comments of the consequent expression
        //
        // ```javascript
        // if (cond1) expr1; // comment A
        // else if (cond2) expr2; // comment A
        // else expr3;
        //
        // if (cond1) expr1; /* comment */ else  expr2;
        //
        // if (cond1) expr1; /* b */
        // else if (cond2) expr2; /* b */
        // else expr3; /* b*/
        // ```
        if !comment.kind().is_block()
            && !comment.position().is_own_line()
            && !comment.preceding_node().is_none()
        {
            return CommentPlacement::Dangling {
                node: consequent,
                comment,
            };
        }

        // ```javascript
        // if (cond1) expr1;
        //
        // /* comment */ else  expr2;
        //
        // if (cond) expr; /*
        // a multiline comment */
        // else b;
        //
        // if (6) // comment
        // true
        // else // comment
        // {true}
        // ```
        CommentPlacement::Dangling {
            node: if_statement,
            comment,
        }
    }

    match (comment.enclosing_node().kind(), comment.following_node()) {
        (JsSyntaxKind::JS_IF_STATEMENT, Some(following)) => {
            let if_statement = JsIfStatement::unwrap_cast(comment.enclosing_node().clone());

            if let Some(preceding) = comment.preceding_node() {
                // Test if this is a comment right before the condition's `)`
                if comment.following_token().kind() == JsSyntaxKind::R_PAREN {
                    return CommentPlacement::Trailing {
                        node: preceding.clone(),
                        comment,
                    };
                }

                // Handle comments before `else`
                if following.kind() == JsSyntaxKind::JS_ELSE_CLAUSE {
                    let consequent = preceding.clone();
                    let if_statement = comment.enclosing_node().clone();
                    return handle_else_clause(comment, consequent, if_statement);
                }
            }

            // Move comments coming before the `{` inside of the block
            //
            // ```javascript
            // if (cond) /* test */ {
            // }
            // ```
            if let Some(block_statement) = JsBlockStatement::cast_ref(following) {
                return place_block_statement_comment(block_statement, comment);
            }

            // Don't attach comments to empty statements
            // ```javascript
            // if (cond) /* test */ ;
            // ```
            if let Some(preceding) = comment.preceding_node() {
                if JsEmptyStatement::can_cast(following.kind()) {
                    return CommentPlacement::Trailing {
                        node: preceding.clone(),
                        comment,
                    };
                }
            }

            // Move comments coming before an if chain inside the body of the first non chain if.
            //
            // ```javascript
            // if (cond1)  /* test */ if (other) { a }
            // ```
            if let Some(if_statement) = JsIfStatement::cast_ref(following) {
                if let Ok(nested_consequent) = if_statement.consequent() {
                    return place_leading_statement_comment(nested_consequent, comment);
                }
            }

            // Make all comments after the condition's `)` leading comments
            // ```javascript
            // if (5) // comment
            // true
            //
            // ```
            if let Ok(consequent) = if_statement.consequent() {
                if consequent.syntax() == following {
                    return CommentPlacement::Leading {
                        node: following.clone(),
                        comment,
                    };
                }
            }
        }
        (JsSyntaxKind::JS_ELSE_CLAUSE, _) => {
            if let Some(if_statement) = comment
                .enclosing_node()
                .parent()
                .and_then(JsIfStatement::cast)
            {
                if let Ok(consequent) = if_statement.consequent() {
                    return handle_else_clause(
                        comment,
                        consequent.into_syntax(),
                        if_statement.into_syntax(),
                    );
                }
            }
        }
        _ => {
            // fall through
        }
    }

    CommentPlacement::Default(comment)
}

fn handle_while_comment(comment: DecoratedComment<JsLanguage>) -> CommentPlacement<JsLanguage> {
    let (while_statement, following) = match (
        JsWhileStatement::cast_ref(comment.enclosing_node()),
        comment.following_node(),
    ) {
        (Some(while_statement), Some(following)) => (while_statement, following),
        _ => return CommentPlacement::Default(comment),
    };

    if let Some(preceding) = comment.preceding_node() {
        // Test if this is a comment right before the condition's `)`
        if comment.following_token().kind() == JsSyntaxKind::R_PAREN {
            return CommentPlacement::Trailing {
                node: preceding.clone(),
                comment,
            };
        }
    }

    // Move comments coming before the `{` inside of the block
    //
    // ```javascript
    // while (cond) /* test */ {
    // }
    // ```
    if let Some(block) = JsBlockStatement::cast_ref(following) {
        return place_block_statement_comment(block, comment);
    }

    // Don't attach comments to empty statements
    // ```javascript
    // if (cond) /* test */ ;
    // ```
    if let Some(preceding) = comment.preceding_node() {
        if JsEmptyStatement::can_cast(following.kind()) {
            return CommentPlacement::Trailing {
                node: preceding.clone(),
                comment,
            };
        }
    }

    // Make all comments after the condition's `)` leading comments
    // ```javascript
    // while (5) // comment
    // true
    //
    // ```
    if let Ok(body) = while_statement.body() {
        if body.syntax() == following {
            return CommentPlacement::Leading {
                node: body.into_syntax(),
                comment,
            };
        }
    }

    CommentPlacement::Default(comment)
}

fn handle_try_comment(comment: DecoratedComment<JsLanguage>) -> CommentPlacement<JsLanguage> {
    let following = match comment.following_node() {
        Some(following)
            if matches!(
                comment.enclosing_node().kind(),
                JsSyntaxKind::JS_TRY_STATEMENT | JsSyntaxKind::JS_TRY_FINALLY_STATEMENT
            ) =>
        {
            // Move comments before the `catch` or `finally` inside of the body
            // ```javascript
            // try {
            // }
            //  catch(e) {
            // }
            // // Comment 7
            // finally {}
            // ```
            let body = if let Some(catch) = JsCatchClause::cast_ref(following) {
                catch.body()
            } else if let Some(finally) = JsFinallyClause::cast_ref(following) {
                finally.body()
            } else {
                // Use an err, so that the following code skips over it
                Err(rome_rowan::SyntaxError::MissingRequiredChild)
            };

            //
            // ```javascript
            // try {
            // } /* comment catch {
            // }
            // ```
            if let Ok(body) = body {
                return place_block_statement_comment(body, comment);
            }

            following
        }
        Some(following)
            if matches!(
                comment.enclosing_node().kind(),
                JsSyntaxKind::JS_CATCH_CLAUSE | JsSyntaxKind::JS_FINALLY_CLAUSE
            ) =>
        {
            following
        }
        _ => return CommentPlacement::Default(comment),
    };

    // Move comments coming before the `{` inside of the block
    //
    // ```javascript
    // try /* test */ {
    // }
    // ```
    if let Some(block) = JsBlockStatement::cast_ref(following) {
        return place_block_statement_comment(block, comment);
    }

    CommentPlacement::Default(comment)
}

fn handle_for_comment(comment: DecoratedComment<JsLanguage>) -> CommentPlacement<JsLanguage> {
    let enclosing = comment.enclosing_node();

    let is_for_in_or_of = matches!(
        enclosing.kind(),
        JsSyntaxKind::JS_FOR_OF_STATEMENT | JsSyntaxKind::JS_FOR_IN_STATEMENT
    );

    if !is_for_in_or_of && !matches!(enclosing.kind(), JsSyntaxKind::JS_FOR_STATEMENT) {
        return CommentPlacement::Default(comment);
    }

    if comment.position().is_own_line() && is_for_in_or_of {
        CommentPlacement::Leading {
            node: enclosing.clone(),
            comment,
        }
    }
    // Don't attach comments to empty statement
    // ```javascript
    // for /* comment */ (;;);
    // for (;;a++) /* comment */;
    // ```
    else if comment.following_node().map_or(false, |following| {
        JsEmptyStatement::can_cast(following.kind())
    }) {
        if let Some(preceding) = comment.preceding_node() {
            CommentPlacement::Trailing {
                node: preceding.clone(),
                comment,
            }
        } else {
            CommentPlacement::Dangling {
                node: comment.enclosing_node().clone(),
                comment,
            }
        }
    } else {
        CommentPlacement::Default(comment)
    }
}

fn handle_variable_declarator_comment(
    comment: DecoratedComment<JsLanguage>,
) -> CommentPlacement<JsLanguage> {
    let following = match comment.following_node() {
        Some(following) => following,
        None => return CommentPlacement::Default(comment),
    };

    fn is_complex_value(value: &JsSyntaxNode) -> bool {
        matches!(
            value.kind(),
            JsSyntaxKind::JS_OBJECT_EXPRESSION
                | JsSyntaxKind::JS_ARRAY_EXPRESSION
                | JsSyntaxKind::JS_TEMPLATE
                | JsSyntaxKind::TS_OBJECT_TYPE
        )
    }

    let enclosing = comment.enclosing_node();
    match enclosing.kind() {
        JsSyntaxKind::JS_ASSIGNMENT_EXPRESSION | JsSyntaxKind::TS_TYPE_ALIAS_DECLARATION => {
            // Makes all comments preceding objects/arrays/templates or block comments leading comments of these nodes.
            // ```javascript
            // let a = // comment
            // { };
            // ```
            if is_complex_value(following) || !comment.kind().is_line() {
                return CommentPlacement::Leading {
                    node: following.clone(),
                    comment,
                };
            }
        }
        JsSyntaxKind::JS_VARIABLE_DECLARATOR => {
            let variable_declarator = JsVariableDeclarator::unwrap_cast(enclosing.clone());

            match variable_declarator.initializer() {
                // ```javascript
                // let obj2 // Comment
                // = {
                //   key: 'val'
                // }
                // ```
                Some(initializer) if initializer.syntax() == following => {
                    if let Ok(expression) = initializer.expression() {
                        return CommentPlacement::Leading {
                            node: expression.into_syntax(),
                            comment,
                        };
                    }
                }
                _ => {
                    // fall through
                }
            }
        }
        JsSyntaxKind::JS_INITIALIZER_CLAUSE => {
            if let Some(variable_declarator) =
                enclosing.parent().and_then(JsVariableDeclarator::cast)
            {
                // Keep trailing comments with the id for variable declarators. Necessary because the value is wrapped
                // inside of an initializer clause.
                // ```javascript
                // let a = // comment
                //      b;
                // ```
                if !is_complex_value(following)
                    && !JsCommentStyle::is_suppression(comment.piece().text())
                    && comment.kind().is_line()
                    && comment.preceding_node().is_none()
                {
                    if let Ok(id) = variable_declarator.id() {
                        return CommentPlacement::Trailing {
                            node: id.into_syntax(),
                            comment,
                        };
                    }
                }
            }
        }
        _ => {
            // fall through
        }
    }

    CommentPlacement::Default(comment)
}

fn handle_parameter_comment(comment: DecoratedComment<JsLanguage>) -> CommentPlacement<JsLanguage> {
    // Make all own line comments leading comments of the parameter
    // ```javascript
    // function a(
    //   b
    //   // comment
    //   = c
    // )
    // ```
    match comment.enclosing_node().kind() {
        JsSyntaxKind::JS_FORMAL_PARAMETER if comment.position().is_own_line() => {
            return CommentPlacement::Leading {
                node: comment.enclosing_node().clone(),
                comment,
            }
        }
        JsSyntaxKind::JS_INITIALIZER_CLAUSE => {
            if let Some(parameter) = comment
                .enclosing_node()
                .parent()
                .and_then(JsFormalParameter::cast)
            {
                // Keep end of line comments after the `=` trailing comments of the id
                // ```javascript
                // function a(
                //   b = // test
                //     c
                // )
                // ```
                if comment.position().is_end_of_line() && comment.preceding_node().is_none() {
                    if let Ok(binding) = parameter.binding() {
                        return CommentPlacement::Trailing {
                            node: binding.into_syntax(),
                            comment,
                        };
                    }
                } else if comment.position().is_own_line() {
                    return CommentPlacement::Leading {
                        node: parameter.into_syntax(),
                        comment,
                    };
                }
            }
        }
        _ => {
            // fall through
        }
    }

    CommentPlacement::Default(comment)
}

/// Format comments preceding the type parameter name in mapped types on the line above.
///
/// This is achieved by making them dangling comments of the mapped type.
///
/// ```javascript
/// type A = {
///   /* comment */
///   [a in A]: string;
/// }
/// ```
fn handle_mapped_type_comment(
    comment: DecoratedComment<JsLanguage>,
) -> CommentPlacement<JsLanguage> {
    if let (JsSyntaxKind::TS_MAPPED_TYPE, Some(following)) =
        (comment.enclosing_node().kind(), comment.following_node())
    {
        if following.kind() == JsSyntaxKind::TS_TYPE_PARAMETER_NAME {
            return CommentPlacement::Dangling {
                node: comment.enclosing_node().clone(),
                comment,
            };
        }
    }

    CommentPlacement::Default(comment)
}

fn place_leading_statement_comment(
    statement: JsAnyStatement,
    comment: DecoratedComment<JsLanguage>,
) -> CommentPlacement<JsLanguage> {
    match statement {
        JsAnyStatement::JsBlockStatement(block) => place_block_statement_comment(block, comment),
        statement => CommentPlacement::Leading {
            node: statement.into_syntax(),
            comment,
        },
    }
}

fn place_block_statement_comment(
    block_statement: JsBlockStatement,
    comment: DecoratedComment<JsLanguage>,
) -> CommentPlacement<JsLanguage> {
    let first_non_empty = block_statement
        .statements()
        .iter()
        .find(|statement| !matches!(statement, JsAnyStatement::JsEmptyStatement(_)));

    match first_non_empty {
        None => CommentPlacement::Dangling {
            node: block_statement.into_syntax(),
            comment,
        },
        Some(statement) => CommentPlacement::Leading {
            node: statement.into_syntax(),
            comment,
        },
    }
}

/// Returns `true` if `comment` is a [Closure type comment](https://github.com/google/closure-compiler/wiki/Types-in-the-Closure-Type-System)
/// or [TypeScript type comment](https://www.typescriptlang.org/docs/handbook/jsdoc-supported-types.html#type)
pub(crate) fn is_type_comment(comment: &SyntaxTriviaPieceComments<JsLanguage>) -> bool {
    let text = comment.text();

    // Must be a `/**` comment
    if !text.starts_with("/**") {
        return false;
    }

    text.trim_start_matches("/**")
        .trim_end_matches("*/")
        .split_whitespace()
        .any(|word| match word.strip_prefix("@type") {
            Some(after) => after.is_empty() || after.starts_with('{'),
            None => false,
        })
}
