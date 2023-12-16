use cfgrammar::yacc::{
    ast::{self, GrammarAST},
    YaccGrammar, YaccGrammarError, YaccGrammarWarning, YaccKind,
};
use cfgrammar::{PIdx, Span, Spanned};
use lrlex::{CTLexerBuilder, LexBuildError, LexErrorHandler};
use lrpar::{GrammarErrorHandler, LexerTypes};
use lrtable::{statetable::Conflicts, StateGraph, StateTable};
use std::{cell::RefCell, error, fmt, path, rc::Rc, collections::HashSet};
use ariadne::{Report, ReportKind, Label};

const LEX_FILENAME: &str = "erroneous.l";
const YACC_FILENAME: &str = "erroneous.y";

// So we can derive traits on it.
struct LSpan(String, cfgrammar::Span);
struct GSpan(String, cfgrammar::Span);

impl ariadne::Span for LSpan {
    type SourceId = String;
    fn source(&self) -> &String {
        &self.0
    }
    fn start(&self) -> usize {
        self.1.start()
    }

    fn end(&self) -> usize {
        self.1.end()
    }
}
impl ariadne::Span for GSpan {
    type SourceId = String;
    fn source(&self) -> &String {
        &self.0
    }
    fn start(&self) -> usize {
        self.1.start()
    }

    fn end(&self) -> usize {
        self.1.end()
    }
}

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
struct AriadneLexErrorHandler<'a> {
    src: String,
    path: path::PathBuf,
    reports: Vec<Report<'a, LSpan>>,
}

struct AriadneGrammarErrorHandler<'a> {
    src: String,
    path: path::PathBuf,
    err_reports: Vec<Report<'a, GSpan>>,
    warning_reports: Vec<Report<'a, GSpan>>,
    errors: String,
    warnings: String,
    warnings_are_errors: bool,
    //newline_cache: NewlineCache,
}

impl<'a> AriadneLexErrorHandler<'a> {
    fn new() -> Self {
        Self {
            src: String::new(),
            path: path::PathBuf::new(),
            reports: Vec::new(),
        }
    }
}

impl<'a, StorageT, LexemeT, ErrorT, LexerTypesT> LexErrorHandler<LexerTypesT> for AriadneLexErrorHandler<'a>
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
        let path_name = self.path.display().to_string();
        for err in errs {
            let spans = err.spans();
            let span = spans.first().unwrap();
            let report = ariadne::Report::<LSpan>::build(
                    ReportKind::Error, path_name.clone(), span.start(),
                ).with_message(err.to_string());
            self.reports.push(report.finish())
        }
    }

    fn missing_in_lexer(&mut self, missing: &HashSet<String>) {
        let path_name = self.path.display().to_string();
        let mut report = Report::<LSpan>::build(
            ReportKind::Error, path_name, 0,
        ).with_message(
            "The following tokens are used in the grammar but are not defined in the lexer:"
        );
        if !missing.is_empty() {
            let mut iter = missing.iter();
            let mut note = String::from(iter.next().unwrap());
            for n in missing {
                note.push_str(format!(", {}", n).as_str());
            }
            report.set_note(note);
        }
        self.reports.push(report.finish());
    }

    fn missing_in_parser(&mut self, missing: &HashSet<String>) {
        let path_name = self.path.display().to_string();
        let mut report = Report::<LSpan>::build(
            ReportKind::Error, path_name, 0, // 0 not sure what else, EOF probably
        ).with_message(
            "The following tokens are used in the lexer but are not defined in the grammar"
        );
        if !missing.is_empty() {
            let mut iter = missing.iter();
            let mut note = String::from(iter.next().unwrap());
            for n in missing {
                note.push_str(format!(", {}", n).as_str());
            }
            report.set_note(note);
        }
        self.reports.push(report.finish());
    }

    fn results(&self) -> Result<(), Box<dyn error::Error>> {
        if self.reports.is_empty() {
            Ok(())
        } else {
            let mut x: Vec<u8> = vec![];
            let path_name = self.path.display().to_string();
            let mut srcs = ariadne::sources(vec![
                (path_name, self.src.as_str()),
            ]);
            for r in &self.reports {
                r.write(&mut srcs, &mut x)?;
            }
            let s = String::from_utf8(x).unwrap();
           Err(ErrorString(s).into())
        }
    }
}

impl<'a> AriadneGrammarErrorHandler<'a> {
    fn new() -> Self {
        Self {
            src: String::new(),
            path: path::PathBuf::new(),
            errors: String::new(),
            warnings: String::new(),
            warnings_are_errors: false,
            err_reports: vec![],
            warning_reports: vec![],
        }
    }
}

impl<'a, LexerTypesT> GrammarErrorHandler<LexerTypesT> for AriadneGrammarErrorHandler<'a>
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
        let path_name = self.path.display().to_string();
        for w in warnings {
            // FIXME use this with label?
            let spans = w.spans();
            let span = spans.first().unwrap();
            let report = ariadne::Report::<GSpan>::build(
                    ReportKind::Warning, path_name.clone(), span.start(),
                ).with_message(w.to_string());
            self.warning_reports.push(report.finish())
        }
    }
    fn on_grammar_error(&mut self, errs: &[YaccGrammarError]) {
        let path_name = self.path.display().to_string();
        for err in errs {
            let spans = err.spans();
            let span = spans.first().unwrap();
            let report = ariadne::Report::<GSpan>::build(
                    ReportKind::Error, path_name.clone(), span.start(),
                ).with_message(err.to_string());
            self.err_reports.push(report.finish())
        }
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
        let path_name = self.path.display().to_string();

        // I'm not sure yet what of this information is going to be helpful yet.
        // But here is i believe all of or a good amount of the span information related
        // to conflicts, their rules, productions the spans of those and their names.
        //
        // We'll need to figure out what we actually need
        for (r1_prod_idx, r2_prod_idx, _st_idx) in c.rr_conflicts() {
            needs_newline = true;

            let (_r1_prod_names, _r1_prod_spans) = pidx_prods_data(ast, *r1_prod_idx);
            let (_r2_prod_names, _r2_prod_spans) = pidx_prods_data(ast, *r2_prod_idx);

            let r1_rule_idx = grm.prod_to_rule(*r1_prod_idx);
            let r2_rule_idx = grm.prod_to_rule(*r2_prod_idx);
            let r1_span = grm.rule_name_span(r1_rule_idx);
            let r2_span = grm.rule_name_span(r2_rule_idx);
            let _r1_name = grm.rule_name_str(r1_rule_idx);
            let _r2_name = grm.rule_name_str(r2_rule_idx);
            let report = ariadne::Report::<GSpan>::build(
                    ariadne::ReportKind::Error, path_name.clone(), r1_span.start(),
                ).with_message("Reduce/Reduce".to_string())
                .with_label(Label::new(GSpan(path_name.clone(), r1_span)).with_message("1st Reduce"))
                .with_label(Label::new(GSpan(path_name.clone(), r2_span)).with_message("2nd Reduce"));
            self.err_reports.push(report.finish());
        }
        if needs_newline {
            self.errors.push('\n');
        }
        for (s_tok_idx, r_prod_idx, _st_idx) in c.sr_conflicts() {
            let r_rule_idx = grm.prod_to_rule(*r_prod_idx);
            let s_tok_span = grm.token_span(*s_tok_idx).unwrap();
            let _shift_name = grm.token_name(*s_tok_idx).unwrap();
            let _reduce_name = grm.rule_name_str(r_rule_idx);
            let (_r_prod_names, r_prod_spans) = pidx_prods_data(ast, *r_prod_idx);
            let rule_idx = grm.prod_to_rule(*r_prod_idx);
            let rule_span = grm.rule_name_span(rule_idx);
            let report = ariadne::Report::<GSpan>::build(
                    ariadne::ReportKind::Error, path_name.clone(), rule_span.start(),
                ).with_message("Shift/Reduce".to_string())
                .with_label(Label::new(GSpan(path_name.clone(), s_tok_span)).with_message("Shifted"))
                .with_label(Label::new(GSpan(path_name.clone(), rule_span)).with_message("Reduced rule"));
            let report = r_prod_spans.iter().fold(report, |report, span| {
                report.with_label(Label::new(GSpan(path_name.clone(), *span)).with_message("Reduced production"))
            });
            self.err_reports.push(report.finish());
        }
    }

    fn results(&self) -> Result<(), Box<dyn error::Error>> {
        if self.errors.is_empty() {
            Ok(())
        } else if self.warnings.is_empty() {
            let mut x: Vec<u8> = vec![];
            let path_name = self.path.display().to_string();
            let mut srcs = ariadne::sources(vec![
                (path_name, self.src.as_str()),
            ]);
            for r in &self.err_reports {
                r.write(&mut srcs, &mut x)?;
            }
            let s = String::from_utf8(x).unwrap();
            Err(ErrorString(format!("\n{}", s)).into()) 
        } else {
            let mut srcs = ariadne::sources(vec![
                (self.path.display().to_string(), self.src.as_str()),
            ]);

            let (warnings, errors) =
                ({
                    let mut x: Vec<u8> = vec![];
                    for r in &self.warning_reports {
                        r.write(&mut srcs, &mut x)?;
                    }
                    String::from_utf8(x)?
                }, {
                    let mut x: Vec<u8> = vec![];
                    for r in &self.err_reports {
                        r.write(&mut srcs, &mut x)?;
                    }
                    String::from_utf8(x)?
                });
            Err(ErrorString(format!(
                "\nWarnings:\n{}\n\nErrors:\n{}\n",
                warnings, errors
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
    let mut lex_error_handler = AriadneLexErrorHandler::new();
    let grammar_error_handler = Rc::new(RefCell::new(AriadneGrammarErrorHandler::new()));
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
    eprintln!("warnings: {}", (*grammar_error_handler).borrow().warnings);
    // For debugging in case we succeed
    panic!();
//    Ok(())
}
