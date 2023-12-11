use cfgrammar::yacc::{
    ast::{self, GrammarAST},
    YaccGrammar, YaccGrammarError, YaccKind,
};
use lrlex::{CTLexerBuilder, LexBuildError};
use lrtable::{statetable::Conflicts, StateGraph, StateTable};

use std::error::Error;
use std::fmt;
const LEX_FILENAME: &str = "erroneous.l";
const YACC_FILENAME: &str = "erroneous.y";

/// A string which uses `Display` for it's `Debug` impl.
struct ErrorString(String);
impl fmt::Display for ErrorString {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let ErrorString(s) = self;
        write!(f, "{}", s)
    }
}
impl fmt::Debug for ErrorString {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let ErrorString(s) = self;
        write!(f, "{}", s)
    }
}
impl Error for ErrorString {}

fn lex_error(errs: Vec<LexBuildError>) -> Box<dyn Error> {
    ErrorString(format!(
        "Lex error: {}",
        errs.iter()
            .map(|e| format!("{}", e))
            .collect::<Vec<_>>()
            .join("\n")
    ))
    .into()
}

fn grammar_error(errs: Vec<YaccGrammarError>) -> Box<dyn Error> {
    ErrorString(format!(
        "Parse error: {}",
        errs.iter()
            .map(|e| format!("{}", e))
            .collect::<Vec<_>>()
            .join("\n")
    ))
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
        + 'static
        + num_traits::PrimInt
        + num_traits::Unsigned
        + std::fmt::Debug,
{
    let prods = &ast.prods;
    let mut out = String::new();
    let mut needs_newline = false;
    // I'm not sure yet what of this information is going to be helpful yet.
    // But here is i believe all of or a good amount of the span information related
    // to conflicts, their rules, productions the spans of those and their names.
    //
    // We'll need to figure out what we actually need
    if let Some(c) = stable.conflicts() {
        for (r1_prod_idx, r2_prod_idx, _st_idx) in c.rr_conflicts() {
            needs_newline = true;
            if usize::from(*r1_prod_idx) < prods.len() {
                let prod = &prods[usize::from(*r1_prod_idx)];
                let _prod_spans = prod.symbols.iter().map(|sym| match sym {
                    ast::Symbol::Rule(_, span) => span,
                    ast::Symbol::Token(_, span) => span,
                });
            }
            let r1_rule_idx = grm.prod_to_rule(*r1_prod_idx);
            let r2_rule_idx = grm.prod_to_rule(*r2_prod_idx);
            let _r1_span = grm.rule_name_span(r1_rule_idx);
            let _r2_span = grm.rule_name_span(r2_rule_idx);
            let r1_name = grm.rule_name_str(r1_rule_idx);
            let r2_name = grm.rule_name_str(r2_rule_idx);
            out.push_str(format!("Reduce/reduce: {r1_name}/{r2_name}\n").as_str());
        }
        if needs_newline {
            out.push('\n');
        }
        for (s_tok_idx, r_prod_idx, _st_idx) in c.sr_conflicts() {
            let r_rule_idx = grm.prod_to_rule(*r_prod_idx);
            let span2 = grm.token_span(*s_tok_idx);
            let shift_name = grm.token_name(*s_tok_idx).unwrap();
            let reduce_name = grm.rule_name_str(r_rule_idx);
            if usize::from(*r_prod_idx) < prods.len() {
                let prod = &prods[usize::from(*r_prod_idx)];
                let _prod_spans = prod.symbols.iter().map(|sym| match sym {
                    ast::Symbol::Rule(_, span) => span,
                    ast::Symbol::Token(_, span) => span,
                });
            }
            let rule_idx = grm.prod_to_rule(*r_prod_idx);
            let _rule_span = grm.rule_name_span(rule_idx);
            let reduce_rule_name = grm.rule_name_str(rule_idx);
            out.push_str(
                format!(
                    "Shift/Reduce: {:?} Shift: {} Reduce: {} at rule {}\n",
                    span2, shift_name, reduce_name, reduce_rule_name,
                )
                .as_str(),
            );
        }
    }
    ErrorString(out).into()
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
