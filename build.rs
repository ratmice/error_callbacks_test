use cfgrammar::yacc::{
    ast::{self, GrammarAST},
    YaccGrammar, YaccGrammarError, YaccKind,
};
use cfgrammar::{PIdx, Span};
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

fn pidx_prods_data<StorageT>(ast: &GrammarAST, pidx: PIdx<StorageT>) -> (Vec<String>, Vec<Span>)
where
    usize: num_traits::AsPrimitive<StorageT>,
    StorageT:
        std::hash::Hash + 'static + num_traits::PrimInt + num_traits::Unsigned + std::fmt::Debug,
{
    if usize::from(pidx) < ast.prods.len() {
        let prod = &ast.prods[usize::from(pidx)];
        prod.symbols
            .iter()
            .map(|sym| match sym {
                ast::Symbol::Rule(name, span) => (format!("'{}'", name), span),
                ast::Symbol::Token(name, span) => (format!("'{}'", name), span),
            })
            .unzip()
    } else {
        (vec![], vec![])
    }
}

fn on_unexpected_conflicts<StorageT>(
    ast: &GrammarAST,
    grm: &YaccGrammar<StorageT>,
    _sgraph: &StateGraph<StorageT>,
    _stable: &StateTable<StorageT>,
    c: &Conflicts<StorageT>,
) -> Box<dyn Error>
where
    usize: num_traits::AsPrimitive<StorageT>,
    StorageT:
        std::hash::Hash + 'static + num_traits::PrimInt + num_traits::Unsigned + std::fmt::Debug,
{
    let mut out = String::new();
    let mut needs_newline = false;

    out.push('\n');
    // I'm not sure yet what of this information is going to be helpful yet.
    // But here is i believe all of or a good amount of the span information related
    // to conflicts, their rules, productions the spans of those and their names.
    //
    // We'll need to figure out what we actually need
    for (r1_prod_idx, r2_prod_idx, _st_idx) in c.rr_conflicts() {
        needs_newline = true;

        let (r1_prod_names, _r1_prod_spans) = pidx_prods_data(ast, *r1_prod_idx);
        let (r2_prod_names, _r2_prod_spans) = pidx_prods_data(ast, *r2_prod_idx);

        let r1_rule_idx = grm.prod_to_rule(*r1_prod_idx);
        let r2_rule_idx = grm.prod_to_rule(*r2_prod_idx);
        let _r1_span = grm.rule_name_span(r1_rule_idx);
        let _r2_span = grm.rule_name_span(r2_rule_idx);
        let r1_name = grm.rule_name_str(r1_rule_idx);
        let r2_name = grm.rule_name_str(r2_rule_idx);
        out.push_str("Reduce/Reduce:\n");
        out.push_str(format!("\tLeft: {r1_name}\n").as_str());
        out.push_str(format!("\tRight: {r2_name}\n").as_str());
        out.push_str(format!("\tLeft Productions: {}\n", r1_prod_names.join(" ")).as_str());
        out.push_str(format!("\tRight Productions: {}\n", r2_prod_names.join(" ")).as_str());
    }
    if needs_newline {
        out.push('\n');
    }
    for (s_tok_idx, r_prod_idx, _st_idx) in c.sr_conflicts() {
        let r_rule_idx = grm.prod_to_rule(*r_prod_idx);
        let _span2 = grm.token_span(*s_tok_idx);
        let shift_name = grm.token_name(*s_tok_idx).unwrap();
        let reduce_name = grm.rule_name_str(r_rule_idx);
        let (r_prod_names, _r_prod_spans) = pidx_prods_data(ast, *r_prod_idx);
        let rule_idx = grm.prod_to_rule(*r_prod_idx);
        let _rule_span = grm.rule_name_span(rule_idx);
        out.push_str("Shift/Reduce:\n");
        out.push_str(format!("\tShift: {shift_name}\n").as_str());
        out.push_str(format!("\tReduce: {reduce_name}\n").as_str());
        out.push_str(format!("\tReduce Productions: {}\n", r_prod_names.join(" ")).as_str());
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
