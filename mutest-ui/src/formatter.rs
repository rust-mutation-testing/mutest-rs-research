use core::fmt;
use std::collections::HashMap;
use minijinja::{context, Environment};
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style, Theme};
use syntect::parsing::{SyntaxReference, SyntaxSet};

const LAYOUT_HTML_TPL: &str = include_str!("templates/layout.html");
const STYLE: &str = include_str!("styles/style.css");

/// HighlighterOpts contains the syntax reference and theme used by the syntax highlighter.
pub(crate) struct HighlighterOpts {
    pub syntax_set: SyntaxSet,
    pub syntax_reference: SyntaxReference,
    pub theme: Theme,
}

impl HighlighterOpts {
    pub(crate) fn new(syntax_set: SyntaxSet, theme: Theme) -> Self {
        let syntax_reference = syntax_set.find_syntax_by_extension("rs").unwrap().clone();
        HighlighterOpts {
            syntax_set,
            syntax_reference,
            theme,
        }
    }
}

/// DiffType tells the syntax highlighter which type of difference a particular line is.
#[derive(Debug)]
pub(crate) enum DiffType {
    NEW,
    OLD,
    NONE,
}

impl fmt::Display for DiffType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

/// Diff contains all the diff information about a particular line.
pub(crate) struct Diff {
    diff_type: DiffType,
    begin_char: usize,
    end_char: usize,
}

/// LineDiff is a map of line number to diff information. If a line has no diff, it should not be
/// included in the map.
pub(crate) type LineDiff = HashMap<usize, Diff>;

// trait LineDiffMethods {
//     fn add(&mut self, md: &crate::MutationData);
// }
//
// impl LineDiffMethods for LineDiff {
//     fn add(&mut self, md: &crate::MutationData) {
//         for i in md.begin.line..=md.end.line {
//             self.insert(i, )
//         }
//     }
// }

/// LineCount stores the number of old and new lines in a file.
struct LineCount {
    old: usize,
    new: usize,
}

impl LineCount {
    fn new() -> Self {
        LineCount { old: 0, new: 0 }
    }
}

fn format_line_numbers(line_index: &mut LineCount, diff_type: &DiffType) -> String {
    let mut numbers = String::new();
    match diff_type { // TODO: might need to rethink this when displaying multiple mutations
        DiffType::NEW => {
            line_index.new += 1;
            numbers.push_str("<td class=\"numbers\"></td><td class=\"numbers\">");
            numbers.push_str(&line_index.new.to_string());
            numbers.push_str("</td>");
        }
        DiffType::OLD => {
            line_index.old += 1;
            numbers.push_str("<td class=\"numbers\">");
            numbers.push_str(&line_index.new.to_string());
            numbers.push_str("</td><td class=\"numbers\"></td>");
        }
        DiffType::NONE => {
            line_index.new += 1;
            line_index.old += 1;
            numbers.push_str("<td class=\"numbers\">");
            numbers.push_str(&line_index.new.to_string());
            numbers.push_str("</td><td class=\"numbers\">");
            numbers.push_str(&line_index.new.to_string());
            numbers.push_str("</td>");
        }
    }
    numbers
}

fn format_line(opts: &HighlighterOpts, highlighter: &mut HighlightLines, line: &String, diff_type: &DiffType) -> String {
    let mut line_str = String::new();

    line_str.push_str("<td class=\"line-content");
    match diff_type {
        DiffType::NEW | DiffType::OLD => {
            line_str.push(' ');
            line_str.push_str(&diff_type.to_string());
        }
        _ => {}
    }
    line_str.push_str("\">");

    let ranges: Vec<(Style, &str)> = highlighter.highlight_line(line, &opts.syntax_set).unwrap(); // TODO: handle this error correctly.
    for (style, text) in ranges { // TODO: optimise by using classes
        let rgb = style.foreground;
        line_str.push_str("<span style=\"color: rgb(");
        line_str.push_str(&format!("{},{},{}", rgb.r, rgb.g, rgb.b));
        line_str.push_str(")\">");
        line_str.push_str(text);
        line_str.push_str("</span>");
    }

    line_str.push_str("</td>");
    line_str
}

fn highlight_and_diff_line(opts: &HighlighterOpts, highlighter: &mut HighlightLines, line_index: &mut LineCount, line: &String, diff: &Option<&Diff>) -> String {
    let mut tr_str = String::new();
    let mut diff_type = &DiffType::NONE;

    tr_str.push_str("<tr class=\"line-wrapper");
    if let Some(diff) = diff {
        tr_str.push(' ');
        tr_str.push_str(&diff.diff_type.to_string());
        diff_type = &diff.diff_type;
    }
    tr_str.push_str("\">");

    tr_str.push_str(&format_line_numbers(line_index, diff_type));
    tr_str.push_str(&format_line(opts, highlighter, line, diff_type));

    tr_str.push_str("</tr>\n");
    tr_str
}

pub(crate) fn highlight_and_diff2(opts: HighlighterOpts, lines: &Vec<String>, diff: LineDiff) -> Result<String, Box<dyn std::error::Error>> {
    let mut highlighter = HighlightLines::new(&opts.syntax_reference, &opts.theme);
    let mut highlighted = String::new();
    let mut line_indexes = LineCount::new();
    for (i, line) in lines.iter().enumerate() {
        highlighted.push_str(&highlight_and_diff_line(&opts, &mut highlighter, &mut line_indexes, line, &diff.get(&i)))
    }
    Ok(highlighted)
}

pub(crate) fn render_tpl(data: &String) -> String {
    let mut env = Environment::new();
    env.add_template("layout.html", LAYOUT_HTML_TPL).unwrap(); // TODO:
    let template = env.get_template("layout.html").unwrap(); // TODO:
    let render = template.render(context! {
        title => "testing",
        filetree => "none",
        codetable => data,
    }).unwrap().to_string();
    render
}
