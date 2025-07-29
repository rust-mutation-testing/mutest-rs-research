use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::fmt::Write;
use actix_web::http::header::DispositionType::Inline;
use similar::{ChangeTag, TextDiff};
use smallvec::SmallVec;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style, Theme, ThemeSet};
use syntect::parsing::{SyntaxReference, SyntaxSet};
use uuid::Uuid;
use mutest_json::call_graph::{CallGraph, CallGraphInfo};
use mutest_json::{Definition, Idx, Span};
use crate::config::SysDiffType;
use crate::mutations::{Conflict, DetectionStatus, Mutation, Mutations, Range};
use crate::{file_tree, split_lines, CompleteDisplayCallee, DefCallTrace, DefTraceGroup, DisplayCallee, NameOrPath};

/// Calculates the offset of a Range within its wider Conflict. This is crucial for mutations that
/// are fewer lines than the region they are in.
fn calc_char_offset(range: &Range, conflict: &Conflict, lines: &[String]) -> usize {
    let lines_offset = range.line - conflict.start_line;
    let mut char_offset = lines_offset;
    for i in 0..lines_offset {
        char_offset += lines[i].len();
    }
    char_offset + range.char
}

/// Replaces a region in the source string with the replacement string. This creates a new owned
/// string. If you don't need a new owned string consider using String replace_range.
fn replace(source: &String, replacement: &String, start_index: usize, end_index: usize) -> String {
    let capacity = source.len() - (end_index - start_index) + replacement.len();
    let mut replaced = String::with_capacity(capacity);

    replaced.push_str(&source[..start_index]);
    replaced.push_str(replacement);
    replaced.push_str(&source[end_index..]);

    replaced
}

fn get_detection_status(detection_status: &Option<DetectionStatus>) -> &str {
    match detection_status {
        None => "",
        Some(s) => s.as_str(),
    }
}

/// Increments the specified number unless that number is 0 where it returns 0.
fn inc_or_zero(n: usize) -> usize {
    match n {
        0 => 0,
        _ => n + 1,
    }
}

/// Represents the available diff types for the interface. Used by both Line and LineBlock to
/// represent a global (to the line) diff type and a local (to a line segment) diff type.
#[derive(Copy, Clone, Debug)]
pub enum InlineSpanType {
    DiffNew,
    DiffOld,
    DiffUnchanged,
    Definition,
    BlockerDefinition, // a definition that requires content to be moved in order to display
    Call,
    DefAndCall,
}

impl InlineSpanType {
    /// Converts a Similar ChangeTag into a DiffType.
    pub fn from_change_tag(change_tag: ChangeTag) -> InlineSpanType {
        match change_tag {
            ChangeTag::Delete => InlineSpanType::DiffOld,
            ChangeTag::Insert => InlineSpanType::DiffNew,
            ChangeTag::Equal => InlineSpanType::DiffUnchanged,
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            InlineSpanType::DiffNew => "insert",
            InlineSpanType::DiffOld => "remove",
            InlineSpanType::Definition => "definition",
            InlineSpanType::BlockerDefinition => "definition blocker",
            InlineSpanType::DiffUnchanged => "",
            InlineSpanType::Call => "call",
            InlineSpanType::DefAndCall => "def-and-call",
        }
    }
}

/// Contains a line that is to be syntax highlighted and rendered. A line is composed of a number
/// of line blocks, which are individual segments of the whole line.
struct Line {
    /// The global diff type for the line.
    base_highlight: InlineSpanType,
    /// The line blocks that make up the line content.
    blocks: Vec<LineBlock>,
    /// The line number. As 0 is never a line number, 0 is reserved for showing no line number.
    number: usize,
}

impl Line {
    pub fn from_spans(line_text: &String, base_highlight: InlineSpanType, highlights: &[(LineSpan, InlineSpanType)], number: usize) -> Self {
        let mut line = Line {
            base_highlight,
            blocks: vec![],
            number,
        };

        let mut cursor = 0;
        for (highlight_span, highlight_kind) in highlights {
            line.blocks.push(LineBlock {
                text: line_text[cursor..highlight_span.begin].to_owned(),
                highlight: InlineSpanType::DiffUnchanged,
            });
            line.blocks.push(LineBlock {
                text: line_text[highlight_span.begin..highlight_span.end].to_owned(),
                highlight: *highlight_kind,
            });
            cursor = highlight_span.end;
        }

        line.blocks.push(LineBlock {
            text: line_text[cursor..].to_owned(),
            highlight: InlineSpanType::DiffUnchanged,
        });

        line
    }
}

#[derive(Debug)]
struct LineSpan {
    begin: usize,
    end: usize,
}

impl LineSpan {
    pub fn from_span(s: &Span) -> Self {
        LineSpan { begin: s.begin.1 - 1, end: s.end.1 - 1 }
    }

    pub fn from_span_line(span: &Span, line_number: usize, line_length: usize) -> Self {
        if line_number == span.begin.0 && line_number == span.end.0 {
            LineSpan::from_span(span)
        } else if line_number == span.begin.0 {
            LineSpan { begin: span.begin.0 - 1, end: line_length - 1}
        } else if line_number == span.end.0 {
            LineSpan { begin: 0, end: span.end.0 - 1}
        } else {
            LineSpan { begin: 0, end: line_length - 1}
        }
    }
}

/// Contains a segment of a larger line.
struct LineBlock {
    text: String,
    highlight: InlineSpanType,
}

fn write_detection_status_marker(html_out: &mut String, detection_status: &Option<DetectionStatus>) {
    write!(html_out, "<div class=\"detection-status-marker {0}\">{0}</div>", get_detection_status(detection_status));
}

fn write_detection_status_mini_marker(html_out: &mut String, detection_status: &Option<DetectionStatus>) {
    write!(html_out, "<div class=\"detection-status-marker mini {0}\" title=\"{0}\"></div>", get_detection_status(detection_status));
}

fn write_code_tr_open(html_out: &mut String, line_type: &InlineSpanType, detection_status: &Option<DetectionStatus>, line_number: usize, traces_button: bool) {
    write!(html_out, "<tr id=\"line-{line_number}\" class=\"line-wrapper {}\">", line_type.as_str());
    html_out.push_str("<td class=\"line-controls");
    if line_number == 0 {
        html_out.push_str(" new")
    }
    html_out.push_str("\"><div class=\"controls-wrapper\">");
    write_detection_status_mini_marker(html_out, detection_status);
    if traces_button {
        html_out.push_str("<button class=\"show-trace-btn control-button\" title=\"Show call graph traces for this mutation\">");
        write_icon(html_out, "tree.png");
        html_out.push_str("</button>");
    }
    html_out.push_str("</div></td><td class=\"numbers");
    if line_number != 0 {
        write!(html_out, "\">{}", line_number);
    } else {
        html_out.push_str(" new\">");
    }
    html_out.push_str("</td>");
}

fn write_tr_close(html_out: &mut String) {
    html_out.push_str("</tr>");
}

fn highlight_span(html_out: &mut String, style: Style, text: &str) {
    let rgb = style.foreground;
    write!(html_out, "<span style=\"color: rgb({},{},{})\">{}</span>", rgb.r, rgb.g, rgb.b, html_escape::encode_text(text));
}

fn write_icon_with_class_list(html_out: &mut String, icon_name: &str, class_list: &str) {
    write!(html_out, "<img class=\"generic-icon {}\" src=\"/static/icons/{}\" alt=\"\" />", class_list, icon_name);
}

fn write_icon(html_out: &mut String, icon_name: &str) {
    write_icon_with_class_list(html_out, icon_name, "");
}

fn write_mutation_section_header(html_out: &mut String, conflict: &Conflict, i: i32) {
    write!(html_out, "<tr><td colspan=\"3\" class=\"mutation-conflict-header\">{} of {} mutations in region [{}:{}], Click region to show all mutations</td></tr>",
                               i, &conflict.mutations.len(), conflict.start_line + 1, conflict.end_line + 1);
}

/// A cache of all rendered components. Ensures that the renderer only renders most things once
/// in its lifetime.
struct RendererCache {
    /// An array of rendered mutations inside their conflict region indexed against their id.
    mutations: Vec<String>,
    /// The rendered file tree component.
    file_tree: String,
    /// The rendered search popover component.
    search: String,
    /// The fully rendered code sections for each code file stored against their path. This str
    /// does contain the mutations inline for convenience.
    code: HashMap<PathBuf, String>,
}

impl RendererCache {
    pub fn new() -> Self {
        RendererCache {
            mutations: Vec::new(),
            file_tree: String::new(),
            search: String::new(),
            code: HashMap::new()
        }
    }
}

/// A wrapper of all the variables required to perform syntax highlighting. Under syntects own
/// advice, the loading of these variable should only be done once as it is highly performance
/// intensive.
struct RendererSyntaxAssets {
    syntax_set: SyntaxSet,
    syntax_ref: SyntaxReference,
    theme: Theme,
}

impl RendererSyntaxAssets {
    pub fn new(assets_path: &PathBuf) -> Self {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let syntax_ref = syntax_set.find_syntax_by_extension("rs").unwrap().clone();
        let theme = ThemeSet::load_from_folder(assets_path.join("themes")).unwrap().themes["Darcula"].clone();

        RendererSyntaxAssets { syntax_set, syntax_ref, theme }
    }
}

/// A renderer to format and return html pages for the web server.
pub struct Renderer {
    /// A hashmap of source file path to lines.
    source_files: HashMap<PathBuf, Vec<String>>,
    /// A hashmap of source file path to vec of conflict regions.
    source_mutations: Mutations,
    /// A cache of rendered components.
    render_cache: RendererCache,
    /// Variables used for syntax highlighting.
    syntax_highlighter: RendererSyntaxAssets,
}

impl Renderer {
    pub fn new(assets_path: &PathBuf, source_files: HashMap<PathBuf, Vec<String>>, source_mutations: Mutations) -> Self {
        Renderer {
            source_files,
            source_mutations,
            render_cache: RendererCache::new(),
            syntax_highlighter: RendererSyntaxAssets::new(assets_path),
        }
    }

    /// Pre-renders all the mutations inside their conflict regions and stores them in the render
    /// cache. This is done to ensure each mutation is only ever rendered once.
    pub fn cache_mutations(&mut self, diff_type: SysDiffType) {
        self.render_cache.mutations = self.source_mutations.values()
            .flat_map(|cs| cs.iter())
            .flat_map(|c| &c.mutations)
            .map(|_| "".to_owned()).collect::<Vec<String>>();

        for (path, conflicts) in &self.source_mutations {
            for conflict in conflicts {
                let conflict_region_lines = &self.source_files.get(path).unwrap()[conflict.start_line..=conflict.end_line];
                let conflict_region = conflict_region_lines.join("\n");

                for mutation in &conflict.mutations {
                    let start_char_index = calc_char_offset(&mutation.starts, conflict, &conflict_region_lines);
                    let end_char_index = calc_char_offset(&mutation.ends, conflict, &conflict_region_lines);

                    let replaced = replace(&conflict_region, &mutation.replacement, start_char_index, end_char_index);

                    let mut lines: Vec<Line> = Vec::new();
                    let mut mutation_render = String::new();

                    // TODO: refactor
                    match diff_type {
                        SysDiffType::Simple => {
                            let unchanged_start_lines = mutation.starts.line - conflict.start_line;
                            let unchanged_end_lines = conflict.end_line - mutation.ends.line;
                            let mut mutation_end_line_index = conflict_region_lines.len() - unchanged_end_lines - 1;
                            let replaced_lines = split_lines(&replaced);

                            for i in 0..unchanged_start_lines {
                                lines.push(Line {
                                    base_highlight: InlineSpanType::DiffUnchanged,
                                    number: conflict.start_line + i,
                                    blocks: vec![
                                        LineBlock {
                                            highlight: InlineSpanType::DiffUnchanged,
                                            text: conflict_region_lines.get(i).unwrap().to_string(),
                                        }
                                    ]
                                });
                            }

                            if mutation.starts.line == mutation.ends.line {
                                let old_line = conflict_region_lines.get(unchanged_start_lines).unwrap();
                                lines.push(Line {
                                    base_highlight: InlineSpanType::DiffOld,
                                    number: mutation.starts.line,
                                    blocks: vec![
                                        LineBlock {
                                            highlight: InlineSpanType::DiffUnchanged,
                                            text: old_line[0..mutation.starts.char].to_string(),
                                        },
                                        LineBlock {
                                            highlight: InlineSpanType::DiffOld,
                                            text: old_line[mutation.starts.char..mutation.ends.char].to_string(),
                                        },
                                        LineBlock {
                                            highlight: InlineSpanType::DiffUnchanged,
                                            text: old_line[mutation.ends.char..old_line.len()].to_string(),
                                        }
                                    ]
                                });
                            } else {
                                for i in unchanged_start_lines..=mutation_end_line_index {
                                    let line = conflict_region_lines.get(i).unwrap();
                                    let line_number = mutation.starts.line + i - unchanged_start_lines;
                                    if i == unchanged_start_lines {
                                        lines.push(Line {
                                            base_highlight: InlineSpanType::DiffOld,
                                            number: line_number,
                                            blocks: vec![
                                                LineBlock {
                                                    highlight: InlineSpanType::DiffUnchanged,
                                                    text: line[0..mutation.starts.char].to_string(),
                                                },
                                                LineBlock {
                                                    highlight: InlineSpanType::DiffOld,
                                                    text: line[mutation.starts.char..line.len()].to_string(),
                                                }
                                            ]
                                        });
                                    } else if i == mutation_end_line_index {
                                        lines.push(Line {
                                            base_highlight: InlineSpanType::DiffOld,
                                            number: line_number,
                                            blocks: vec![
                                                LineBlock {
                                                    highlight: InlineSpanType::DiffOld,
                                                    text: line[0..mutation.ends.char].to_string(),
                                                },
                                                LineBlock {
                                                    highlight: InlineSpanType::DiffUnchanged,
                                                    text: line[mutation.ends.char..line.len()].to_string(),
                                                }
                                            ]
                                        });
                                    } else {
                                        lines.push(Line {
                                            base_highlight: InlineSpanType::DiffOld,
                                            number: line_number,
                                            blocks: vec![
                                                LineBlock {
                                                    highlight: InlineSpanType::DiffOld,
                                                    text: line.to_string(),
                                                }
                                            ]
                                        });
                                    }
                                }
                            }

                            // replaced end line index for replaced_lines
                            mutation_end_line_index = replaced_lines.len() - unchanged_end_lines - 1;

                            if mutation.starts.line == mutation.ends.line && conflict_region_lines.len() == replaced_lines.len() {
                                let new_line = replaced_lines.get(unchanged_start_lines).unwrap();
                                let mutation_end_offset = mutation.starts.char + mutation.replacement.len();
                                lines.push(Line {
                                    base_highlight: InlineSpanType::DiffNew,
                                    number: 0, // as line numbers cannot be 0, 0 is used as null
                                    blocks: vec![
                                        LineBlock {
                                            highlight: InlineSpanType::DiffUnchanged,
                                            text: new_line[0..mutation.starts.char].to_string(),
                                        },
                                        LineBlock {
                                            highlight: InlineSpanType::DiffNew,
                                            text: new_line[mutation.starts.char..mutation_end_offset].to_string(),
                                        },
                                        LineBlock {
                                            highlight: InlineSpanType::DiffUnchanged,
                                            text: new_line[mutation_end_offset..new_line.len()].to_string(),
                                        }
                                    ]
                                });
                            } else {
                                for i in unchanged_start_lines..=mutation_end_line_index {
                                    let line = replaced_lines.get(i).unwrap();
                                    if i == unchanged_start_lines {
                                        lines.push(Line {
                                            base_highlight: InlineSpanType::DiffNew,
                                            number: 0,
                                            blocks: vec![
                                                LineBlock {
                                                    highlight: InlineSpanType::DiffUnchanged,
                                                    text: line[0..mutation.starts.char].to_string(),
                                                },
                                                LineBlock {
                                                    highlight: InlineSpanType::DiffNew,
                                                    text: line[mutation.starts.char..line.len()].to_string(),
                                                }
                                            ]
                                        });
                                    } else if i == mutation_end_line_index {
                                        let end_index = split_lines(&mutation.replacement).last().unwrap().len();
                                        lines.push(Line {
                                            base_highlight: InlineSpanType::DiffNew,
                                            number: 0,
                                            blocks: vec![
                                                LineBlock {
                                                    highlight: InlineSpanType::DiffNew,
                                                    text: line[0..end_index].to_string(),
                                                },
                                                LineBlock {
                                                    highlight: InlineSpanType::DiffUnchanged,
                                                    text: line[end_index..line.len()].to_string(),
                                                }
                                            ]
                                        });
                                    } else {
                                        lines.push(Line {
                                            base_highlight: InlineSpanType::DiffNew,
                                            number: 0,
                                            blocks: vec![
                                                LineBlock {
                                                    highlight: InlineSpanType::DiffNew,
                                                    text: line.to_string(),
                                                }
                                            ]
                                        });
                                    }
                                }
                            }

                            mutation_end_line_index = conflict_region_lines.len() - unchanged_end_lines - 1;

                            for i in mutation_end_line_index + 1..conflict_region_lines.len() {
                                lines.push(Line {
                                    base_highlight: InlineSpanType::DiffUnchanged,
                                    number: conflict.start_line + i,
                                    blocks: vec![
                                        LineBlock {
                                            highlight: InlineSpanType::DiffUnchanged,
                                            text: conflict_region_lines.get(i).unwrap().to_string(),
                                        }
                                    ]
                                });
                            }
                        },
                        SysDiffType::Advanced => {
                            let diff = TextDiff::from_lines(&conflict_region, &replaced);
                            let changes: Vec<_> = diff.iter_all_changes().collect();
                            let mut original_lines_counter = 0;

                            for (i, change) in changes.iter().enumerate() {
                                match change.tag() {
                                    ChangeTag::Delete => {
                                        // Find corresponding insert
                                        if let Some(insert_change) = changes.get(i + 1)
                                            .filter(|c| c.tag() == ChangeTag::Insert) {

                                            let word_diff = TextDiff::from_words(change.value(), insert_change.value());
                                            let mut line = Line {
                                                base_highlight: InlineSpanType::DiffOld,
                                                number: conflict.start_line + original_lines_counter,
                                                blocks: vec![],
                                            };
                                            original_lines_counter += 1;

                                            let mut inline_delete_value = String::new();
                                            let mut inline_unchange_value = String::new();

                                            for word_change in word_diff.iter_all_changes() {
                                                match word_change.tag() {
                                                    ChangeTag::Delete => {
                                                        inline_delete_value.push_str(word_change.value());

                                                        if inline_unchange_value != "" {
                                                            line.blocks.push(LineBlock {
                                                                highlight: InlineSpanType::DiffUnchanged,
                                                                text: inline_unchange_value.clone(),
                                                            });
                                                            inline_unchange_value = String::new();
                                                        }
                                                    }
                                                    ChangeTag::Insert => {}
                                                    ChangeTag::Equal => {
                                                        inline_unchange_value.push_str(word_change.value());

                                                        if inline_delete_value != "" {
                                                            line.blocks.push(LineBlock {
                                                                highlight: InlineSpanType::DiffOld,
                                                                text: inline_delete_value.clone(),
                                                            });
                                                            inline_delete_value = String::new();
                                                        }
                                                    }
                                                }
                                            }

                                            if inline_unchange_value != "" {
                                                line.blocks.push(LineBlock {
                                                    highlight: InlineSpanType::DiffUnchanged,
                                                    text: inline_unchange_value.clone(),
                                                });
                                            }

                                            if inline_delete_value != "" {
                                                line.blocks.push(LineBlock {
                                                    highlight: InlineSpanType::DiffOld,
                                                    text: inline_delete_value.clone(),
                                                });
                                            }

                                            lines.push(line);
                                        } else {
                                            lines.push(Line {
                                                base_highlight: InlineSpanType::DiffOld,
                                                number: conflict.start_line + original_lines_counter,
                                                blocks: vec![
                                                    LineBlock {
                                                        highlight: InlineSpanType::DiffUnchanged,
                                                        text: change.value().to_string(),
                                                    }
                                                ]
                                            });
                                            original_lines_counter += 1;
                                        }
                                    }
                                    ChangeTag::Insert => {
                                        // TODO: this only fetches line above, not the actual corresponding delete line
                                        if let Some(delete_change) = changes.get(i - 1)
                                            .filter(|c| c.tag() == ChangeTag::Delete) {

                                            let word_diff = TextDiff::from_words(delete_change.value(), change.value());
                                            let mut line = Line {
                                                base_highlight: InlineSpanType::DiffNew,
                                                number: 0,
                                                blocks: vec![],
                                            };

                                            let mut inline_insert_value = String::new();
                                            let mut inline_unchange_value = String::new();

                                            for word_change in word_diff.iter_all_changes() {
                                                match word_change.tag() {
                                                    ChangeTag::Delete => {}
                                                    ChangeTag::Insert => {
                                                        inline_insert_value.push_str(word_change.value());

                                                        if inline_unchange_value != "" {
                                                            line.blocks.push(LineBlock {
                                                                highlight: InlineSpanType::DiffUnchanged,
                                                                text: inline_unchange_value.clone(),
                                                            });
                                                            inline_unchange_value = String::new();
                                                        }
                                                    }
                                                    ChangeTag::Equal => {
                                                        inline_unchange_value.push_str(word_change.value());

                                                        if inline_insert_value != "" {
                                                            line.blocks.push(LineBlock {
                                                                highlight: InlineSpanType::DiffNew,
                                                                text: inline_insert_value.clone(),
                                                            });
                                                            inline_insert_value = String::new();
                                                        }
                                                    }
                                                }
                                            }

                                            if inline_unchange_value != "" {
                                                line.blocks.push(LineBlock {
                                                    highlight: InlineSpanType::DiffUnchanged,
                                                    text: inline_unchange_value.clone(),
                                                });
                                            }

                                            if inline_insert_value != "" {
                                                line.blocks.push(LineBlock {
                                                    highlight: InlineSpanType::DiffNew,
                                                    text: inline_insert_value.clone(),
                                                });
                                            }

                                            lines.push(line);
                                        } else {
                                            lines.push(Line {
                                                base_highlight: InlineSpanType::DiffNew,
                                                number: 0,
                                                blocks: vec![
                                                    LineBlock {
                                                        highlight: InlineSpanType::DiffUnchanged,
                                                        text: change.value().to_string(),
                                                    }
                                                ]
                                            });
                                        }
                                    }
                                    ChangeTag::Equal => {
                                        lines.push(Line {
                                            base_highlight: InlineSpanType::DiffUnchanged,
                                            number: conflict.start_line + original_lines_counter,
                                            blocks: vec![
                                                LineBlock {
                                                    highlight: InlineSpanType::DiffUnchanged,
                                                    text: change.value().to_string(),
                                                }
                                            ]
                                        });
                                        original_lines_counter += 1;
                                    }
                                }
                            }
                        },
                    }

                    self.render_mutation(&mut mutation_render, mutation, lines);
                    self.render_cache.mutations[mutation.mutation_id] = mutation_render;
                }
            }
        }
    }

    /// Renders a mutation inside its conflict region. Uses the Line struct to determine how each
    /// line should be diffed.
    fn render_mutation(&self, html_out: &mut String, mutation: &Mutation, lines: Vec<Line>) {
        let mut highlighter = HighlightLines::new(&self.syntax_highlighter.syntax_ref, &self.syntax_highlighter.theme);

        for (idx, line) in lines.into_iter().enumerate() {
            let detection = if idx == 0 {
                &mutation.detection_status
            } else {
                &None
            };

            self.render_line(html_out, &mut highlighter, idx, &line, detection);
        }
    }

    fn render_line(&self, html_out: &mut String, mut highlighter: &mut HighlightLines, idx: usize, line: &Line, detection: &Option<DetectionStatus>) {
        write_code_tr_open(html_out, &line.base_highlight, detection, inc_or_zero(line.number), idx == 0);
        html_out.push_str("<td class=\"line-content\">");
        for line_block in &line.blocks {
            self.highlight_block(line_block, html_out, &mut highlighter);
        }
        html_out.push_str("</td>");
        write_tr_close(html_out);
    }

    /// Determines if the block requires inline diff highlighting and the passes the text on to
    /// be syntax highlighted.
    fn highlight_block(&self, line_block: &LineBlock, html_out: &mut String, highlighter: &mut HighlightLines) {
        match line_block.highlight {
            InlineSpanType::DiffUnchanged => {},
            _ => {
                let _ = write!(html_out, "<span class=\"inline-diff {}\">", line_block.highlight.as_str());
            },
        }

        self.highlight_line(html_out, highlighter, &line_block.text);

        match line_block.highlight {
            InlineSpanType::DiffUnchanged => {}
            _ => html_out.push_str("</span>"),
        }
    }

    /// Runs the highlighting process for a given line.
    fn highlight_line(&self, html_out: &mut String, highlighter: &mut HighlightLines, line: &str) {
        let ranges: Vec<(Style, &str)> = highlighter.highlight_line(&line, &self.syntax_highlighter.syntax_set).unwrap();
        for (style, text) in ranges {
            highlight_span(html_out, style, text);
        }
    }

    /// Caches the file tree into the render_cache of the Renderer.
    pub fn cache_file_tree(&mut self, ft: file_tree::FileTree) {
        let mut file_tree = String::new();
        file_tree.push_str("<div id=\"file-tree-wrapper\" class=\"file-tree-wrapper\"><div class=\"file-tree-header\">");
        file_tree.push_str("<button id=\"file-tree-tab-btn\" class=\"nav-button selected\" title=\"Show the file tree\">");
        write_icon(&mut file_tree, "folder.png");
        file_tree.push_str("</button>");
        file_tree.push_str("<button id=\"traces-tab-btn\" class=\"nav-button\" title=\"Show the call graph traces\">");
        write_icon(&mut file_tree, "tree.png");
        file_tree.push_str("</button>");
        file_tree.push_str("<div class=\"spacer\"></div>");
        file_tree.push_str("<button id=\"left-pane-hide-btn\" class=\"nav-button\">");
        write_icon(&mut file_tree, "sidebar.png");
        file_tree.push_str("</button></div>");
        write!(file_tree, "<div id=\"file-tree-tab\" class=\"file-tree-container\"><ul id=\"file-tree\" class=\"file-tree\" data-session-id=\"{}\">", Uuid::new_v4());
        for node in ft.children() {
            self.render_file_tree_node(&mut file_tree, node, 0, "/file/");
        }
        file_tree.push_str("</ul></div><div id=\"traces-tab\" class=\"file-tree-container hidden\">");
        file_tree.push_str("<p class=\"default-text\">Click on a call graph traces icon <span class=\"inline-icon\">");
        write_icon(&mut file_tree, "tree.png");
        file_tree.push_str("</span> to show traces in this tab.</p>");
        file_tree.push_str("</div></div>");
        self.render_cache.file_tree = file_tree;
    }

    /// Recursively renders all nodes within a file tree node.
    fn render_file_tree_node(&self, html_out: &mut String, node: &file_tree::Node, indentation_level: usize, current_path_str: &str) {
        let exp = if node.is_folder() {
            "expanded"
        } else {
            ""
        };

        write!(html_out, "<li class=\"ft-node {}\"><div style=\"--level:{};\" class=\"node-content-wrapper\"><button class=\"toggle\">", exp, indentation_level);
        write_icon_with_class_list(html_out, "chevron-right.png", "collapsed");
        write_icon_with_class_list(html_out, "chevron-down.png", "expanded");
        html_out.push_str("</button>");

        html_out.push_str("<a class=\"node-value-wrapper");
        if !node.is_folder() {
            write!(html_out, " file\" href=\"{}{}", current_path_str, node.value());
        }
        html_out.push_str("\">");

        let path = if !node.is_folder() {
            PathBuf::from(format!("{}{}", current_path_str, node.value()).replace("/file/", ""))
        } else {
            PathBuf::new()
        };

        html_out.push_str("<div class=\"node-icon\">");
        if node.is_folder() {
            match node.value() {
                "src" => write_icon(html_out, "folder-blue.png"),
                _ => write_icon(html_out, "folder.png"),
            }
        } else {
            let mut detected = true;
            'outer: for conflicts in self.source_mutations.get(&path).unwrap() {
                for mutation in &conflicts.mutations {
                    match mutation.detection_status {
                        Some(DetectionStatus::Detected) => {},
                        Some(DetectionStatus::Undetected) | Some(DetectionStatus::Crashed) | Some(DetectionStatus::Timeout) | None => {
                            detected = false;
                            break 'outer;
                        }
                    }
                }
            }
            match detected {
                true => write_icon(html_out, "ferris_party_64.png"),
                false => write_icon(html_out, "ferris_worried_64.png"),
            }
        }
        html_out.push_str("</div>");

        html_out.push_str("<div class=\"node-value\">");
        html_out.push_str(node.value());
        html_out.push_str("</div>");
        if !node.is_folder() {
            let mutation_count = self.source_mutations.get(&path).unwrap()
                .iter().flat_map(|c| &c.mutations).collect::<Vec<_>>().len();
            write!(html_out, "<div class=\"no-mutations\">{}</div>", mutation_count);
        }
        html_out.push_str("</div></a>");

        if node.is_folder() {
            html_out.push_str("<ul class=\"file-tree\">");
            for child in node.children() {
                self.render_file_tree_node(html_out, child, indentation_level + 1, &format!("{}{}/", current_path_str, node.value()));
            }
            html_out.push_str("</ul>");
        } else {
            html_out.push_str("<ul class=\"file-tree\">");
            for conflict in self.source_mutations.get(&path).unwrap() {
                for mutation in &conflict.mutations {
                    let escaped = html_escape::encode_text(&mutation.name);
                    write!(html_out, "<li class=\"ft-mutation\" data-mutation-id=\"{}\"><div style=\"--level:{};\" class=\"mutation-name-wrapper\" title=\"{escaped}\">", mutation.display_id(), indentation_level);
                    write_detection_status_mini_marker(html_out, &mutation.detection_status);
                    write!(html_out, "<div class=\"mid\">{}</div><div class=\"mutation-name\">{escaped}</div></div></li>", mutation.display_id());
                }
            }
            html_out.push_str("</ul>");
        }

        html_out.push_str("</li>");
    }

    /// Caches the search pane into the render_cache of the Renderer.
    pub fn cache_search(&mut self) {
        let mut search = String::new();
        search.push_str("<div class=\"search-frame-content-blocker hidden\">");
        search.push_str("<div class=\"search-frame-wrapper\">");
        search.push_str("<div id=\"search-popover\" class=\"search-frame main-search-wrapper\"><div class=\"search-input\">");
        search.push_str("<img class=\"generic-icon\" src=\"/static/icons/magnify.png\" alt=\"magnifying glass\" />");
        search.push_str("<input id=\"search-input\" class=\"search-input-field\" type=\"search\" placeholder=\"Search to filter mutations\" />");
        search.push_str("<div class=\"checkbox-wrapper\">");
        search.push_str("<input id=\"use-regex\" class=\"checkbox\" type=\"checkbox\" />");
        search.push_str("<label for=\"use-regex\" class=\"checkbox-label\">Use regex</label>");
        search.push_str("</div></div><div class=\"mutations-wrapper\">");

        for (path, conflicts) in &self.source_mutations {
            for conflict in conflicts {
                for mutation in &conflict.mutations {
                    let escaped = html_escape::encode_text(&mutation.name);
                    write!(search, "<div class=\"search-mutation\" data-mutation-id=\"{}\" data-file-path=\"/file/{}\" title=\"{escaped}\">", mutation.display_id(), path.display());
                    write_detection_status_mini_marker(&mut search, &mutation.detection_status);
                    write!(search, "<div class=\"mid\">{}</div><div class=\"mutation-name\">{escaped}</div></div>", mutation.display_id());
                }
            }
        }

        search.push_str("</div></div></div></div>");
        self.render_cache.search = search;
    }

    /// Determines if a path exists inside the renderers source code pool.
    pub fn valid_path(&self, path: &PathBuf) -> bool {
        match self.source_files.get(path) {
            None => false,
            Some(_) => true
        }
    }

    /// Renders the default start page containing the file tree and some tips for using the app.
    pub fn render_start(&self) -> String {
        self.render_start_with_error("")
    }

    /// Renders the start page containing the file tree, some tips for using the app and the
    /// specified error message.
    pub fn render_start_with_error(&self, error_str: &str) -> String {
        let mut render = String::from("<!DOCTYPE html><html><head>");
        render.push_str("<meta charset=\"utf-8\">");
        if error_str == "" {
            render.push_str("<title>Mutest Report - Tips and Tricks</title>");
        } else {
            write!(render, "<title>Mutest Report - Error: {}</title>", error_str);
        }
        render.push_str("<link rel=\"stylesheet\" href=\"/static/styles/style.css\" />");
        render.push_str("<script type=\"module\" src=\"/static/scripts/file-tree.js\"></script>");
        render.push_str("<script type=\"module\" src=\"/static/scripts/search.js\"></script>");
        render.push_str("<link rel=\"icon\" type=\"image/x-icon\" href=\"/static/icons/ferris_64.png\">");
        render.push_str("</head><body>");
        render.push_str(&self.render_cache.search);
        render.push_str(&self.render_cache.file_tree);
        render.push_str("<div class=\"code-wrapper\"><div class=\"code-header\">");
        render.push_str("<button id=\"left-pane-show-btn\" class=\"nav-button hidden\">");
        write_icon(&mut render, "sidebar.png");
        render.push_str("</button></div><div class=\"main-code-wrapper help-wrapper\"><div class=\"help\">");
        write_icon_with_class_list(&mut render, "ferris_bg.webp", "ferris-bg");
        render.push_str("<div class=\"help-text\"><span class=\"key\">/</span> open search</div>");
        render.push_str("</div></div>");
        render.push_str("<div class=\"status-bar\"><div class=\"spacer\"></div>");
        render.push_str("<div class=\"status-text\"><span class=\"key\">/</span> to search</div>");
        render.push_str("</div></div></div></body></html>");
        render
    }

    /// Renders the file (route: /file) document for a specific file.
    pub fn render_file(&mut self, path: &PathBuf) -> String {
        let mut render = String::from("<!DOCTYPE html><html><head>");
        render.push_str("<meta charset=\"utf-8\">");
        write!(render, "<title>Mutest Report - {}</title>", path.file_name().unwrap().display());
        render.push_str("<link rel=\"stylesheet\" href=\"/static/styles/style.css\" />");
        render.push_str("<script type=\"module\" src=\"/static/scripts/code-main.js\"></script>");
        render.push_str("<script type=\"module\" src=\"/static/scripts/search.js\"></script>");
        render.push_str("<link rel=\"icon\" type=\"image/x-icon\" href=\"/static/icons/ferris_64.png\">");
        render.push_str("</head><body>");
        render.push_str(&self.render_cache.search);
        render.push_str(&self.render_cache.file_tree);
        match self.render_cache.code.get(path) {
            Some(code) => render.push_str(code),
            None => self.cache_file_code(&mut render, path),
        }
        render.push_str("</body></html>");
        render
    }

    /// Caches file code section and immediately writes cache to html_out.
    fn cache_file_code(&mut self, html_out: &mut String, path: &PathBuf) {
        // TODO: refactor

        let mut render = String::new();
        let file_lines = self.source_files.get(path).unwrap();
        let mut file_conflicts = &self.source_mutations.get(path).unwrap()[..];
        let mut highlighter = HighlightLines::new(&self.syntax_highlighter.syntax_ref, &self.syntax_highlighter.theme);
        let mut mutation_changer = String::from("<div id=\"changer\" class=\"mutation-changer hidden\"><div class=\"mutation-changer-nav\"><h2 class=\"window-title\">Mutation Changer</h2><button id=\"mutation-changer-close-btn\" class=\"nav-button\">");
        write_icon(&mut mutation_changer, "x-mark.png");
        mutation_changer.push_str("</button></div><div id=\"changer-regions\" class=\"mutations-wrapper\">");
        let standard_columns = String::from("<colgroup><col span=\"1\" style=\"width: 40px;\"><col span=\"1\" style=\"width: 50px;\"><col span=\"1\" style=\"width: auto;\"></colgroup>");
        let changer_columns = String::from("<colgroup><col span=\"1\" style=\"width: 50px;\"><col span=\"1\" style=\"width: auto;\"></colgroup>");

        render.push_str("<div class=\"code-wrapper\"><div class=\"code-header\">");
        render.push_str("<button id=\"left-pane-show-btn\" class=\"nav-button hidden\">");
        write_icon(&mut render, "sidebar.png");
        render.push_str("</button><div class=\"file-name\">");
        write_icon(&mut render, "ferris_64.png");
        write!(render, "{}</div></div>", path.file_name().unwrap().display());
        render.push_str("<div class=\"main-code-wrapper\"><table id=\"code-table\" class=\"main-code-table hidden\">");
        render.push_str(&standard_columns);

        let mut file_lines_iter = 0..file_lines.len();
        // for mut i in 0..file_lines.len() {
        while let Some(i) = file_lines_iter.next() {
            if let Some(conflict) = file_conflicts.first() {
                if conflict.start_line == i {
                    for _ in conflict.start_line..conflict.end_line { file_lines_iter.next(); }
                    let section_name = format!("conflict-{}", Uuid::new_v4());
                    write!(render, "<tbody id=\"{}\" class=\"{} mutation-region", conflict.mutations[0].display_id(), &section_name);
                    if conflict.mutations.len() > 1 {
                        render.push_str(" mutation-conflict-region");
                    }
                    render.push_str("\">");
                    if conflict.mutations.len() > 1 {
                        write_mutation_section_header(&mut render, conflict, 1);
                    }
                    render.push_str(&self.render_cache.mutations[conflict.mutations.first().unwrap().mutation_id]);
                    render.push_str("</tbody>");
                    if conflict.mutations.len() > 1 {
                        let mut i = 2;
                        for mutation in &conflict.mutations[1..] {
                            render.push_str(&format!("<tbody id=\"{}\" class=\"{} mutation-conflict-region hidden\">", mutation.display_id(), &section_name));

                            write_mutation_section_header(&mut render, conflict, i);

                            render.push_str(&self.render_cache.mutations[mutation.mutation_id]);
                            render.push_str("</tbody>");

                            i += 1;
                        }

                        // adding mutations to the mutation changer interface
                        mutation_changer.push_str("<div id=\"");
                        mutation_changer.push_str(&section_name);
                        mutation_changer.push_str("\" class=\"mutations\">");
                        for mutation in &conflict.mutations {
                            mutation_changer.push_str("<div class=\"mutation-content-wrapper\">");
                            mutation_changer.push_str(&format!("<h2 class=\"mutation-name\"><span class=\"mutation-id\">{}</span> {}</h2>",
                                                               mutation.display_id(), html_escape::encode_text(&mutation.name).as_ref()));
                            write_detection_status_marker(&mut mutation_changer, &mutation.detection_status);
                            mutation_changer.push_str(&format!("<div class=\"mutation-wrapper\" data-target-class=\"{}\" data-mutation-id=\"{}\"><table class=\"no-status no-line-wrapper\">{}<tbody>{}</tbody></table></div></div>",
                                                               &section_name, mutation.display_id(), &changer_columns, &self.render_cache.mutations[mutation.mutation_id]));
                        }
                        mutation_changer.push_str("</div>");
                    }

                    file_conflicts = &file_conflicts[1..];
                    continue;
                }
            }

            write_code_tr_open(&mut render, &InlineSpanType::DiffUnchanged, &None, i + 1, false);
            render.push_str("<td class=\"line-content\">");
            self.highlight_line(&mut render, &mut highlighter, &file_lines[i]);
            render.push_str("</td>");
            write_tr_close(&mut render);
        }

        mutation_changer.push_str("</div></div>");
        write!(render, "</table></div><div class=\"status-bar\"><div class=\"status-text\">{}</div><div class=\"spacer\"></div><div class=\"status-text\"><span class=\"key\">/</span> to search</div></div></div>", path.display());
        render.push_str(&mutation_changer);

        html_out.push_str(&render);
        self.render_cache.code.insert(path.clone(), render);
    }

    pub fn render_call_traces_component(&self, html_out: &mut String, mutation_id: u32, call_traces: &Vec<DefTraceGroup>, call_graph: &CallGraphInfo) {
        write!(html_out, "<ul class=\"file-tree\">");
        for call_trace in call_traces {
            let entry_point = &call_graph.call_graph.entry_points[call_trace.entry_point_id];
            let escaped_entry = html_escape::encode_text(&entry_point.path);

            write!(html_out, "<li class=\"ft-node expanded\"><div style=\"--level:0;\" class=\"node-content-wrapper\"><button class=\"toggle\">");
            write_icon_with_class_list(html_out, "chevron-right.png", "collapsed");
            write_icon_with_class_list(html_out, "chevron-down.png", "expanded");
            write!(html_out, "</button><div class=\"node-value-wrapper\"><div class=\"node-value\" title=\"{escaped_entry}\">{escaped_entry}</div></div></div><ul class=\"file-tree\">");

            for nested_trace in &call_trace.nested_traces {
                let mut href = String::new();
                write!(&mut href, "/trace?mutation_id={mutation_id}&entry_point_id={}&definition_ids=", entry_point.entry_point_id.as_index());

                let mut content = String::new();

                for nested_call in nested_trace {
                    if content != String::new() {
                        content.push_str(" > ");
                    }
                    let nested_callee = &call_graph.definitions[*nested_call];
                    write!(&mut href, "{},", nested_callee.def_id.as_index());
                    write!(&mut content, "{}", html_escape::encode_text(&nested_callee.path.clone().unwrap_or("unknown_path_err".parse().unwrap())));
                }

                write!(html_out, "<li class=\"ft-text\"><div style=\"--level:1;\" class=\"text-wrapper\"><div class=\"text-icon\">");
                write_icon(html_out, "tree.png");
                write!(html_out, "</div><a class=\"text-link\" href=\"{href}\" title=\"{escaped_entry} > {content}\">{content}</a></div></li>");
            }
            write!(html_out, "</ul></li>");
        }
        write!(html_out, "</ul>");
    }

    pub fn render_trace(&self, mutation_id: u32, callees: Vec<DisplayCallee>) -> Result<String, Box<dyn std::error::Error>> {
        let mut render = String::from("<!DOCTYPE html><html><head>");
        render.push_str("<meta charset=\"utf-8\">");
        write!(render, "<title>Mutest Report - Viewing Trace for Mutation {mutation_id}</title>");
        render.push_str("<link rel=\"stylesheet\" href=\"/static/styles/style.css\" />");
        render.push_str("<script type=\"module\" src=\"/static/scripts/search.js\"></script>");
        render.push_str("<script type=\"module\" src=\"/static/scripts/trace-main.js\"></script>");
        render.push_str("<link rel=\"icon\" type=\"image/x-icon\" href=\"/static/icons/ferris_64.png\">");
        render.push_str("</head><body>");
        render.push_str(&self.render_cache.search);
        render.push_str(&self.render_cache.file_tree);

        // TODO: this is repeated code from above so can be refactored.
        let standard_columns = String::from("<colgroup><col span=\"1\" style=\"width: 40px;\"><col span=\"1\" style=\"width: 50px;\"><col span=\"1\" style=\"width: auto;\"></colgroup>");
        render.push_str("<div class=\"code-wrapper\"><div class=\"code-header\">");
        render.push_str("<button id=\"left-pane-show-btn\" class=\"nav-button hidden\">");
        write_icon(&mut render, "sidebar.png");
        render.push_str("</button><div class=\"file-name\">");
        write_icon(&mut render, "ferris_64.png");
        write!(render, "Trace for Mutation {mutation_id}</div></div>");
        render.push_str("<div class=\"main-code-wrapper\"><table id=\"code-table\" class=\"main-code-table hidden\">");
        render.push_str(&standard_columns);
        for callee in callees {
            match callee {
                DisplayCallee::Incomplete(d) => {
                    write!(render, "<tr><td></td><td></td><td class=\"file-header\"><p class=\"generic-text\">Definition <span class=\"inline-code\">{}</span> calls ", html_escape::encode_text(&d.caller_path));

                    match d.callee_name_or_path {
                        NameOrPath::Name(s) => write!(render, "<span class=\"inline-code function\">{}</span></p></td></tr>", html_escape::encode_text(&s)),
                        NameOrPath::Path(s) => write!(render, "definition <span class=\"inline-code\">{}</span></p></td></tr>", html_escape::encode_text(&s)),
                        NameOrPath::Err => write!(render, "<span class=\"inline-code\">{}</span></p></td></tr>", html_escape::encode_text("<anonymous>")),
                    };

                    render.push_str("<tr><td></td><td></td><td class=\"error-wrapper\">");
                    write_icon(&mut render, "error.png");
                    render.push_str("<h3 class=\"error-text\">Unable to load source file</h3>");
                    render.push_str("</td></tr>");
                }
                DisplayCallee::Complete(mut d) => {
                    write!(render, "<tr><td></td><td></td><td class=\"file-header\"><a class=\"file-path\" href=\"{href}?line_number={line_number}\">{callee_path}</a><p class=\"generic-text\">",
                           href = PathBuf::from("/file").join(&d.caller_def_span.path).display(),
                           line_number = d.caller_def_span.begin.0,
                           callee_path = d.caller_def_span.path.display(),
                    );

                    match d.caller_name_or_path {
                        NameOrPath::Name(s) => write!(render, "<span class=\"inline-code function\">{}</span> calls ", html_escape::encode_text(&s)),
                        NameOrPath::Path(s) => write!(render, "Definition <span class=\"inline-code\">{}</span> calls ", html_escape::encode_text(&s)),
                        NameOrPath::Err => write!(render, "<span class=\"inline-code\">{}</span> calls ", html_escape::encode_text("<anonymous>")),
                    };

                    match d.callee_name_or_path {
                        NameOrPath::Name(s) => write!(render, "<span class=\"inline-code function\">{}</span></p></td></tr>", html_escape::encode_text(&s)),
                        NameOrPath::Path(s) => write!(render, "definition <span class=\"inline-code\">{}</span></p></td></tr>", html_escape::encode_text(&s)),
                        NameOrPath::Err => write!(render, "<span class=\"inline-code\">{}</span></p></td></tr>", html_escape::encode_text("<anonymous>")),
                    };

                    match self.source_files.get(&d.caller_def_span.path) {
                        Some(source_file) => {
                            let mut highlighter = HighlightLines::new(&self.syntax_highlighter.syntax_ref, &self.syntax_highlighter.theme);

                            let mut nudge = 0;
                            while source_file[d.caller_def_span.begin.0 - 1 - (nudge + 1)].trim_start().starts_with("#[") {
                                nudge += 1;
                            }

                            render.push_str("<tbody>");

                            let mut line_number = d.caller_def_span.begin.0 - nudge;
                            for line in &source_file[d.caller_def_span.begin.0 - 1 - nudge..=d.snippet_end.0 - 1] {
                                let mut line_highlights = SmallVec::<[(LineSpan, InlineSpanType);1]>::new();

                                if (d.caller_def_span.begin.0..=d.caller_def_span.end.0).contains(&line_number) {
                                    let highlight_type = if d.caller_def_span.begin.1 == 1 || line[..d.caller_def_span.begin.1 - 1].trim_start().len() > 0 {
                                        InlineSpanType::BlockerDefinition
                                    } else {
                                        InlineSpanType::Definition
                                    };

                                    line_highlights.push((LineSpan::from_span_line(&d.caller_def_span, line_number, line.len()), highlight_type));
                                }

                                while let Some(call_span) = d.call_spans.first() && (call_span.begin.0..=call_span.end.0).contains(&line_number) {
                                    if (d.caller_def_span.begin.0..=d.caller_def_span.end.0).contains(&line_number) &&
                                        d.caller_def_span.begin == call_span.begin && d.caller_def_span.end == call_span.end {
                                        line_highlights[0].1 = InlineSpanType::DefAndCall;
                                        d.call_spans.remove(0);
                                        continue;
                                    }

                                    line_highlights.push((LineSpan::from_span_line(&call_span, line_number, line.len()), InlineSpanType::Call));

                                    if line_number == call_span.end.0 {
                                        d.call_spans.remove(0);
                                    }
                                }

                                self.render_line(&mut render, &mut highlighter, 1, &Line::from_spans(line, InlineSpanType::DiffUnchanged, &line_highlights, line_number), &None);
                                line_number += 1;
                            }
                            render.push_str("</tbody>");
                        }
                        None => {
                            render.push_str("<tr><td></td><td></td><td class=\"error-wrapper\">");
                            write_icon(&mut render, "error.png");
                            render.push_str("<h3 class=\"error-text\">Unable to load source file</h3>");
                            render.push_str("</td></tr>");
                        }
                    }
                }
                DisplayCallee::Mutated(d) => {
                    write!(render, "<tr><td></td><td></td><td class=\"file-header\"><a class=\"file-path\" href=\"{href}\">{target_path}</a><p class=\"generic-text\">Mutation <span class=\"inline-code\">{mutation_id}</span> in <span class=\"inline-code function\">{target_name}</span></p></td></tr>",
                        href = PathBuf::from("/file").join(&d.target_span.path).display(),
                        target_path = d.target_span.path.display(),
                        mutation_id = d.mutation_id.0,
                        target_name = d.target_name,
                    );

                    if let Some(source_file) = self.source_files.get(&d.target_span.path) {
                        render.push_str("<tbody>");
                        let mut highlighter = HighlightLines::new(&self.syntax_highlighter.syntax_ref, &self.syntax_highlighter.theme);
                        let mut line_number = d.target_span.begin.0;
                        for line in &source_file[d.target_span.begin.0 - 1..d.snippet_end.0 - 1] {
                            write_code_tr_open(&mut render, &InlineSpanType::DiffUnchanged, &None, line_number, false);
                            render.push_str("<td class=\"line-content\">");
                            self.highlight_line(&mut render, &mut highlighter, line);
                            render.push_str("</td>");
                            write_tr_close(&mut render);
                            line_number += 1;
                        }
                        render.push_str("</tbody><tbody class=\"mutation\">");
                        render.push_str(&self.render_cache.mutations[d.mutation_id.as_index()]);
                        render.push_str("</tbody>");
                    }
                }
            }
        }
        write!(render, "</tbody></table></div><div class=\"status-bar\"><div class=\"status-text\">Trace for Mutation {mutation_id}</div><div class=\"spacer\"></div><div class=\"status-text\"><span class=\"key\">/</span> to search</div></div></div>");
        render.push_str("</body></html>");
        Ok(render)
    }
}