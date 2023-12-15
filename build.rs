use cfgrammar::yacc::{
    ast::{self, GrammarAST},
    YaccGrammar, YaccGrammarError, YaccGrammarWarning, YaccKind,
};
use cfgrammar::{NewlineCache, PIdx, Span, Spanned};
use lrlex::{CTLexerBuilder, LexBuildError, LexErrorHandler};
use lrpar::{GrammarErrorHandler, LexerTypes};
use lrtable::{statetable::Conflicts, StateGraph, StateTable};
use std::{cell::RefCell, error, fmt, path, rc::Rc, collections::HashSet};

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
impl error::Error for ErrorString {}
struct TestLexErrorHandler {
    src: String,
    path: path::PathBuf,
    errors: String,
    _newline_cache: NewlineCache,
}

struct TestGrammarErrorHandler {
    src: String,
    path: path::PathBuf,
    errors: String,
    warnings: String,
    warnings_are_errors: bool,
    newline_cache: NewlineCache,
}

impl TestLexErrorHandler {
    fn new() -> Self {
        Self {
            src: String::new(),
            path: path::PathBuf::new(),
            errors: String::new(),
            _newline_cache: NewlineCache::new(),
        }
    }
}

impl<StorageT, LexemeT, ErrorT, LexerTypesT> LexErrorHandler<LexerTypesT> for TestLexErrorHandler
where
    LexerTypesT: LexerTypes<LexErrorT = ErrorT, LexemeT = LexemeT, StorageT = StorageT>,
    StorageT: Copy + 'static,
    usize: num_traits::AsPrimitive<StorageT>,
{
    fn lexer_path(&mut self, path: &path::Path) {
        self.path = path.to_owned();
    }
    fn lexer_src(&mut self, src: &str) {
        self.src = src.to_owned()
    }

    fn on_lex_build_error(&mut self, errs: &[LexBuildError]) {
        self.errors.push_str(
            format!(
                "Lex error: {}",
                errs.iter()
                    .map(|e| format!("{}", e))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
            .as_str(),
        );
    }

    fn missing_in_lexer(&mut self, missing: &HashSet<String>) {
        self.errors.push_str("The following tokens are used in the grammar but are not defined in the lexer:\n");
        for n in missing {
            self.errors.push_str(format!("    {}", n).as_str());
        }
    }

    fn missing_in_parser(&mut self, missing: &HashSet<String>) {
        self.errors.push_str("The following tokens are used in the lexer but are not defined in the grammar:\n");
        for n in missing {
            self.errors.push_str(format!("    {}", n).as_str());
        }
    }

    fn results(&self) -> Result<(), Box<dyn error::Error>> {
        if self.errors.is_empty() {
            Ok(())
        } else {
            Err(ErrorString(format!("\n{}\n", self.errors)).into())
        }
    }
}

impl TestGrammarErrorHandler {
    fn new() -> Self {
        Self {
            src: String::new(),
            path: path::PathBuf::new(),
            errors: String::new(),
            warnings: String::new(),
            warnings_are_errors: false,
            newline_cache: NewlineCache::new(),
        }
    }
}
fn spanned_fmt(x: &dyn Spanned, inc: &str, line_cache: &NewlineCache) -> String {
    if let Some((line, column)) = line_cache.byte_to_line_num_and_col_num(inc, x.spans()[0].start())
    {
        format!("{} at line {line} column {column}", x)
    } else {
        format!("{}", x)
    }
}

impl<LexerTypesT> GrammarErrorHandler<LexerTypesT> for TestGrammarErrorHandler
where
    LexerTypesT: LexerTypes,
    usize: num_traits::AsPrimitive<LexerTypesT::StorageT>,
{
    fn warnings_are_errors(&mut self, flag: bool) {
        self.warnings_are_errors = flag;
    }

    fn grammar_src(&mut self, src: &str) {
        self.src = src.to_owned();
    }
    fn grammar_path(&mut self, path: &path::Path) {
        self.path = path.to_owned();
    }
    fn on_grammar_warning(&mut self, warnings: &[YaccGrammarWarning]) {
        if warnings.len() > 1 {
            // Indent under the "Error:" prefix.
            self.warnings.push_str(
                format!(
                    "\n\t{}",
                    warnings
                        .iter()
                        .map(|w| spanned_fmt(w, &self.src, &self.newline_cache))
                        .collect::<Vec<_>>()
                        .join("\n\t")
                )
                .as_str(),
            )
        } else {
            self.warnings.push_str(
                spanned_fmt(warnings.first().unwrap(), &self.src, &self.newline_cache).as_str(),
            )
        };
    }
    fn on_grammar_error(&mut self, errs: &[YaccGrammarError]) {
        self.errors.push_str(
            format!(
                "Parse error: {}",
                errs.iter()
                    .map(|e| format!("{}", e))
                    .collect::<Vec<_>>()
                    .join("\n")
            )
            .as_str(),
        )
    }

    fn on_unexpected_conflicts(
        &mut self,
        ast: &GrammarAST,
        grm: &YaccGrammar<LexerTypesT::StorageT>,
        _sgraph: &StateGraph<LexerTypesT::StorageT>,
        _stable: &StateTable<LexerTypesT::StorageT>,
        c: &Conflicts<LexerTypesT::StorageT>,
    ) where
        usize: num_traits::AsPrimitive<LexerTypesT::StorageT>,
        LexerTypesT::StorageT:
            std::hash::Hash + 'static + num_traits::PrimInt + num_traits::Unsigned + fmt::Debug,
    {
        let mut needs_newline = false;

        self.errors.push('\n');
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
            self.errors.push_str("Reduce/Reduce:\n");
            self.errors
                .push_str(format!("\tLeft: {r1_name}\n").as_str());
            self.errors
                .push_str(format!("\tRight: {r2_name}\n").as_str());
            self.errors
                .push_str(format!("\tLeft Productions: {}\n", r1_prod_names.join(" ")).as_str());
            self.errors
                .push_str(format!("\tRight Productions: {}\n", r2_prod_names.join(" ")).as_str());
        }
        if needs_newline {
            self.errors.push('\n');
        }
        for (s_tok_idx, r_prod_idx, _st_idx) in c.sr_conflicts() {
            let r_rule_idx = grm.prod_to_rule(*r_prod_idx);
            let _span2 = grm.token_span(*s_tok_idx);
            let shift_name = grm.token_name(*s_tok_idx).unwrap();
            let reduce_name = grm.rule_name_str(r_rule_idx);
            let (r_prod_names, _r_prod_spans) = pidx_prods_data(ast, *r_prod_idx);
            let rule_idx = grm.prod_to_rule(*r_prod_idx);
            let _rule_span = grm.rule_name_span(rule_idx);
            self.errors.push_str("Shift/Reduce:\n");
            self.errors
                .push_str(format!("\tShift: {shift_name}\n").as_str());
            self.errors
                .push_str(format!("\tReduce: {reduce_name}\n").as_str());
            self.errors
                .push_str(format!("\tReduce Productions: {}\n", r_prod_names.join(" ")).as_str());
        }
    }

    fn results(&self) -> Result<(), Box<dyn error::Error>> {
        if self.errors.is_empty() {
            Ok(())
        } else if self.warnings.is_empty() {
            Err(ErrorString(format!("\n{}\n", self.errors)).into())
        } else {
            Err(ErrorString(format!(
                "\nWarnings:\n{}\n\n Errors:\n{}\n",
                self.warnings, self.errors
            ))
            .into())
        }
    }
}

fn pidx_prods_data<StorageT>(ast: &GrammarAST, pidx: PIdx<StorageT>) -> (Vec<String>, Vec<Span>)
where
    usize: num_traits::AsPrimitive<StorageT>,
    StorageT: std::hash::Hash + 'static + num_traits::PrimInt + num_traits::Unsigned + fmt::Debug,
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

fn main() -> Result<(), Box<dyn error::Error>> {
    let mut lex_error_handler = TestLexErrorHandler::new();
    let grammar_error_handler = Rc::new(RefCell::new(TestGrammarErrorHandler::new()));
    let geh = Rc::clone(&grammar_error_handler);

    CTLexerBuilder::new()
        .error_handler(&mut lex_error_handler)
        .lrpar_config(move |pb| {
            pb.yacckind(YaccKind::Grmtools)
                .error_handler(geh.clone())
                .grammar_in_src_dir(YACC_FILENAME)
                .unwrap()
        })
        .lexer_in_src_dir(LEX_FILENAME)
        .unwrap()
        .build()?;
    println!("warnings: {}", (*grammar_error_handler).borrow().warnings);
    Ok(())
}
