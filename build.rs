use lrlex::{
    CTLexerBuilder, LexBuildError, DefaultLexerTypes
};
use lrpar::LexerTypes;
use cfgrammar::yacc::YaccGrammar;
use lrtable::{StateGraph, StateTable, statetable::Conflicts};
use cfgrammar::yacc::YaccKind;
use cfgrammar::yacc::YaccGrammarError;
use std::error::Error;

const LEX_FILENAME: &'static str = "erroneous.l";
const YACC_FILENAME: &'static str = "erroneous.y";

fn lex_error(errs: Vec<LexBuildError>) -> Box<dyn Error> {
    format!("Lex error: {}", errs.iter().map(|e| format!("{}", e)).collect::<Vec<_>>().join("\n")).into()
}

fn grammar_error(errs: Vec<YaccGrammarError>) -> Box<dyn Error> {
    format!("Parse error: {}", errs.iter().map(|e| format!("{}", e)).collect::<Vec<_>>().join("\n")).into()
}

fn on_unexpected_conflicts(
 grm:   &YaccGrammar,
 _sgraph:   &StateGraph<<DefaultLexerTypes as LexerTypes>::StorageT>,
 stable:   &StateTable<<DefaultLexerTypes as LexerTypes>::StorageT>,
 _conflicts:   &Conflicts<<DefaultLexerTypes as LexerTypes>::StorageT>,
) -> Box<dyn Error> {
    let mut out = String::new();
    if let Some(c) = stable.conflicts() {
        for (r1_prod_idx, r2_prod_idx, _st_idx) in c.rr_conflicts() {
           out.push_str(format!("Reduce/reduce: {:?} {:?}\n", r1_prod_idx.0, r2_prod_idx.0).as_str());

        }
        for (s_tok_idx, r_prod_idx, _st_idx) in c.sr_conflicts() {
            let r_rule_idx = grm.prod_to_rule(*r_prod_idx);
            let span2 = grm.token_span(*s_tok_idx);
            let shift_name = grm.token_name(*s_tok_idx).unwrap();
            let reduce_name = grm.rule_name_str(r_rule_idx);
            out.push_str(format!("Shift/Reduce: {:?} Shift: {} Reduce: {}\n", span2, shift_name, reduce_name).as_str());
        }
    }
    out.into()
}
fn main() -> Result<(), Box<dyn Error>> {
    CTLexerBuilder::new()
        .lrpar_config(|pb| {
            pb
            .yacckind(YaccKind::Grmtools)
            .on_grammar_error(&grammar_error)
            .on_unexpected_conflicts(&on_unexpected_conflicts)
            .grammar_in_src_dir(YACC_FILENAME).unwrap()
        })
        .lexer_in_src_dir(LEX_FILENAME).unwrap()
        .on_lex_build_error(&lex_error)
        .build()?;
    Ok(())
}
