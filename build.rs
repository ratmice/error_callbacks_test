use cfgrammar::yacc::ast::{self, GrammarAST};
use cfgrammar::yacc::YaccGrammar;
use cfgrammar::yacc::YaccGrammarError;
use cfgrammar::yacc::YaccKind;
use cfgrammar::Span;
use lrlex::{CTLexerBuilder, DefaultLexerTypes, LexBuildError};
use lrpar::LexerTypes;
use lrtable::{statetable::Conflicts, StateGraph, StateTable};
use num_traits;
use std::error::Error;
const LEX_FILENAME: &'static str = "erroneous.l";
const YACC_FILENAME: &'static str = "erroneous.y";

fn lex_error(errs: Vec<LexBuildError>) -> Box<dyn Error> {
    format!(
        "Lex error: {}",
        errs.iter()
            .map(|e| format!("{}", e))
            .collect::<Vec<_>>()
            .join("\n")
    )
    .into()
}

fn grammar_error(errs: Vec<YaccGrammarError>) -> Box<dyn Error> {
    format!(
        "Parse error: {}",
        errs.iter()
            .map(|e| format!("{}", e))
            .collect::<Vec<_>>()
            .join("\n")
    )
    .into()
}

fn on_unexpected_conflicts<StorageT>(
    ast: &GrammarAST,
    grm: &YaccGrammar<StorageT>,
    _sgraph: &StateGraph<StorageT>,
    stable: &StateTable<StorageT>,
    _conflicts: &Conflicts<StorageT>,
) -> Box<dyn Error>
where
    usize: num_traits::AsPrimitive<StorageT>,
    StorageT: std::hash::Hash
        + std::cmp::Eq
        + std::marker::Copy
        + 'static
        + num_traits::PrimInt
        + num_traits::Unsigned
        + std::fmt::Debug,
{
    let prods = &ast.prods;
    let mut out = String::new();
    // I'm not sure yet what of this information is going to be helpful yet.
    // But here is i believe all of or a good amount of the span information related
    // to conflicts, their rules, productions the spans of those and their names.
    //
    // We'll need to figure out what we actually need
    if let Some(c) = stable.conflicts() {
        for (r1_prod_idx, r2_prod_idx, _st_idx) in c.rr_conflicts() {
            if usize::from(*r1_prod_idx) < prods.len() {
                let prod = &prods[usize::from(*r1_prod_idx)];
                let prod_spans = prod.symbols.iter().map(|sym| match sym {
                    ast::Symbol::Rule(_, span) => span,
                    ast::Symbol::Token(_, span) => span,
                });
            }
            let r1_rule = grm.prod_to_rule(*r1_prod_idx);
            let r2_rule = grm.prod_to_rule(*r2_prod_idx);
            let r1_span = grm.rule_name_span(r1_rule);
            let r2_span = grm.rule_name_span(r2_rule);

            out.push_str(format!("Reduce/reduce: {:?} {:?}\n", r1_prod_idx, r2_prod_idx).as_str());
        }
        for (s_tok_idx, r_prod_idx, _st_idx) in c.sr_conflicts() {
            let r_rule_idx = grm.prod_to_rule(*r_prod_idx);
            let span2 = grm.token_span(*s_tok_idx);
            let shift_name = grm.token_name(*s_tok_idx).unwrap();
            let reduce_name = grm.rule_name_str(r_rule_idx);
            if usize::from(*r_prod_idx) < prods.len() {
                let prod = &prods[usize::from(*r_prod_idx)];
                let prod_spans = prod.symbols.iter().map(|sym| match sym {
                    ast::Symbol::Rule(_, span) => span,
                    ast::Symbol::Token(_, span) => span,
                });
            }
            let rule = grm.prod_to_rule(*r_prod_idx);
            let rule_span = grm.rule_name_span(rule);
            out.push_str(
                format!(
                    "Shift/Reduce: {:?} Shift: {} Reduce: {}\n",
                    span2, shift_name, reduce_name
                )
                .as_str(),
            );
        }
    }
    out.into()
}
fn main() -> Result<(), Box<dyn Error>> {
    CTLexerBuilder::new()
        .lrpar_config(|pb| {
            pb.yacckind(YaccKind::Grmtools)
                .on_grammar_error(&grammar_error)
                .on_unexpected_conflicts(&on_unexpected_conflicts)
                .grammar_in_src_dir(YACC_FILENAME)
                .unwrap()
        })
        .lexer_in_src_dir(LEX_FILENAME)
        .unwrap()
        .on_lex_build_error(&lex_error)
        .build()?;
    Ok(())
}
