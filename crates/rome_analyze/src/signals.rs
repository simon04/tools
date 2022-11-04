use crate::{
    categories::ActionCategory,
    context::RuleContext,
    registry::{RuleLanguage, RuleRoot},
    rule::Rule,
    AnalyzerDiagnostic, AnalyzerOptions, Queryable, RuleGroup, ServiceBag,
};
use rome_console::{markup, MarkupBuf};
use rome_diagnostics::file::FileSpan;
use rome_diagnostics::v2::advice::CodeSuggestionAdvice;
use rome_diagnostics::v2::Category;
use rome_diagnostics::{file::FileId, Applicability, CodeSuggestion};
use rome_rowan::{AstNode, BatchMutation, BatchMutationExt, Language, TriviaPieceKind};
use std::iter::FusedIterator;
use std::vec::IntoIter;

/// Event raised by the analyzer when a [Rule](crate::Rule)
/// emits a diagnostic, a code action, or both
pub trait AnalyzerSignal<L: Language> {
    fn diagnostic(&self) -> Option<AnalyzerDiagnostic>;
    fn action(&self) -> Option<AnalyzerActionIter<L>>;
}

/// Simple implementation of [AnalyzerSignal] generating a [AnalyzerDiagnostic] from a
/// provided factory function
pub(crate) struct DiagnosticSignal<F> {
    factory: F,
}

impl<F> DiagnosticSignal<F>
where
    F: Fn() -> AnalyzerDiagnostic,
{
    pub(crate) fn new(factory: F) -> Self {
        Self { factory }
    }
}

impl<L: Language, F> AnalyzerSignal<L> for DiagnosticSignal<F>
where
    F: Fn() -> AnalyzerDiagnostic,
{
    fn diagnostic(&self) -> Option<AnalyzerDiagnostic> {
        Some((self.factory)())
    }

    fn action(&self) -> Option<AnalyzerActionIter<L>> {
        None
    }
}

/// Code Action object returned by the analyzer, generated from a [crate::RuleAction]
/// with additional information about the rule injected by the analyzer
///
/// This struct can be converted into a [CodeSuggestion] and injected into
/// a diagnostic emitted by the same signal
#[derive(Debug)]
pub struct AnalyzerAction<L: Language> {
    pub group_name: &'static str,
    pub rule_name: &'static str,
    pub file_id: FileId,
    pub category: ActionCategory,
    pub applicability: Applicability,
    pub message: MarkupBuf,
    pub mutation: BatchMutation<L>,
}

pub struct AnalyzerActionIter<L: Language> {
    file_id: FileId,
    analyzer_actions: IntoIter<AnalyzerAction<L>>,
}

impl<L: Language> AnalyzerActionIter<L> {
    pub fn new(file_id: FileId, actions: Vec<AnalyzerAction<L>>) -> Self {
        Self {
            file_id,
            analyzer_actions: actions.into_iter(),
        }
    }
}

impl<L: Language> Iterator for AnalyzerActionIter<L> {
    type Item = AnalyzerAction<L>;

    fn next(&mut self) -> Option<Self::Item> {
        self.analyzer_actions.next()
    }
}

impl<L: Language> FusedIterator for AnalyzerActionIter<L> {}

impl<L: Language> ExactSizeIterator for AnalyzerActionIter<L> {
    fn len(&self) -> usize {
        self.analyzer_actions.len()
    }
}

#[derive(Debug)]
pub struct AnalyzerMutation<L: Language> {
    pub message: MarkupBuf,
    pub mutation: BatchMutation<L>,
    pub category: ActionCategory,
    pub rule_name: String,
}

pub struct CodeSuggestionAdviceIter<L: Language> {
    iter: IntoIter<AnalyzerAction<L>>,
}

impl<L: Language> Iterator for CodeSuggestionAdviceIter<L> {
    type Item = CodeSuggestionAdvice<MarkupBuf>;

    fn next(&mut self) -> Option<Self::Item> {
        let action = self.iter.next()?;
        let (_, suggestion) = action.mutation.as_text_edits().unwrap_or_default();
        Some(CodeSuggestionAdvice {
            applicability: action.applicability.clone(),
            msg: action.message.clone(),
            suggestion,
        })
    }
}

impl<L: Language> FusedIterator for CodeSuggestionAdviceIter<L> {}

impl<L: Language> ExactSizeIterator for CodeSuggestionAdviceIter<L> {
    fn len(&self) -> usize {
        self.iter.len()
    }
}

pub struct CodeSuggestionIter<L: Language> {
    file_id: FileId,
    iter: IntoIter<AnalyzerAction<L>>,
}

pub struct CodeSuggestionItem<'a> {
    pub category: ActionCategory,
    pub suggestion: CodeSuggestion,
    pub rule_name: &'a str,
}

impl<L: Language> Iterator for CodeSuggestionIter<L> {
    type Item = CodeSuggestionItem<'static>;

    fn next(&mut self) -> Option<Self::Item> {
        let action = self.iter.next()?;
        let (range, suggestion) = action.mutation.as_text_edits().unwrap_or_default();

        Some(CodeSuggestionItem {
            rule_name: action.rule_name,
            category: action.category,
            suggestion: CodeSuggestion {
                span: FileSpan {
                    file: self.file_id,
                    range,
                },
                applicability: action.applicability.clone(),
                msg: action.message.clone(),
                suggestion,
                labels: vec![],
            },
        })
    }
}

impl<L: Language> FusedIterator for CodeSuggestionIter<L> {}

impl<L: Language> ExactSizeIterator for CodeSuggestionIter<L> {
    fn len(&self) -> usize {
        self.iter.len()
    }
}

impl<L: Language> AnalyzerActionIter<L> {
    /// Returns an iterator
    pub fn into_code_suggestion_advices(self) -> CodeSuggestionAdviceIter<L> {
        CodeSuggestionAdviceIter {
            iter: self.analyzer_actions.into_iter(),
        }
    }

    pub fn into_code_suggestions(self) -> CodeSuggestionIter<L> {
        CodeSuggestionIter {
            file_id: self.file_id,
            iter: self.analyzer_actions.into_iter(),
        }
    }
}

/// Analyzer-internal implementation of [AnalyzerSignal] for a specific [Rule](crate::registry::Rule)
pub(crate) struct RuleSignal<'phase, R: Rule> {
    file_id: FileId,
    root: &'phase RuleRoot<R>,
    query_result: <<R as Rule>::Query as Queryable>::Output,
    state: R::State,
    services: &'phase ServiceBag,
    options: AnalyzerOptions,
}

impl<'phase, R> RuleSignal<'phase, R>
where
    R: Rule + 'static,
{
    pub(crate) fn new(
        file_id: FileId,
        root: &'phase RuleRoot<R>,
        query_result: <<R as Rule>::Query as Queryable>::Output,
        state: R::State,
        services: &'phase ServiceBag,
        options: AnalyzerOptions,
    ) -> Self {
        Self {
            file_id,
            root,
            query_result,
            state,
            services,
            options,
        }
    }
}

impl<'bag, R> AnalyzerSignal<RuleLanguage<R>> for RuleSignal<'bag, R>
where
    R: Rule,
{
    fn diagnostic(&self) -> Option<AnalyzerDiagnostic> {
        let ctx =
            RuleContext::new(&self.query_result, self.root, self.services, &self.options).ok()?;

        R::diagnostic(&ctx, &self.state).map(|diag| diag.into_analyzer_diagnostic(self.file_id))
    }

    fn action(&self) -> Option<AnalyzerActionIter<RuleLanguage<R>>> {
        let ctx =
            RuleContext::new(&self.query_result, self.root, self.services, &self.options).ok()?;
        let mut actions = Vec::new();
        if let Some(action) = R::action(&ctx, &self.state) {
            actions.push(AnalyzerAction {
                group_name: <R::Group as RuleGroup>::NAME,
                rule_name: R::METADATA.name,
                file_id: self.file_id,
                category: action.category,
                applicability: action.applicability,
                mutation: action.mutation,
                message: action.message,
            });
        };
        let node_to_suppress = R::can_suppress(&ctx, &self.state);
        let suppression_node = node_to_suppress.and_then(|suppression_node| {
            let ancestor = suppression_node.node().ancestors().find_map(|node| {
                if node
                    .first_token()
                    .map(|token| {
                        token
                            .leading_trivia()
                            .pieces()
                            .any(|trivia| trivia.is_newline())
                    })
                    .unwrap_or(false)
                {
                    Some(node)
                } else {
                    None
                }
            });
            if ancestor.is_some() {
                ancestor
            } else {
                Some(ctx.root().syntax().clone())
            }
        });
        let suppression_action = suppression_node.and_then(|suppression_node| {
            let first_token = suppression_node.first_token();
            let rule = format!(
                "lint({}/{})",
                <R::Group as RuleGroup>::NAME,
                R::METADATA.name
            );
            let mes = format!("// rome-ignore {}: suppressed", rule);

            first_token.and_then(|first_token| {
                let trivia = vec![
                    (TriviaPieceKind::Newline, "\n"),
                    (TriviaPieceKind::SingleLineComment, mes.as_str()),
                    (TriviaPieceKind::Newline, "\n"),
                ];
                let mut mutation = ctx.root().begin();
                let new_token = first_token.with_leading_trivia(trivia.clone());

                mutation.replace_token_discard_trivia(first_token, new_token);
                Some(AnalyzerAction {
                    group_name: <R::Group as RuleGroup>::NAME,
                    rule_name: R::METADATA.name,
                    file_id: self.file_id,
                    category: ActionCategory::QuickFix,
                    applicability: Applicability::Always,
                    mutation,
                    message: markup! { "Suppress rule " {rule} }.to_owned(),
                })
            })
        });
        if let Some(suppression_action) = suppression_action {
            actions.push(suppression_action);
        }
        Some(AnalyzerActionIter::new(self.file_id, actions))
    }
}
