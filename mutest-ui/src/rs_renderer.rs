use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use minijinja::filters::split;
use similar::{Algorithm, ChangeTag, TextDiff, TextDiffConfig};
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style, Theme};
use syntect::parsing::{SyntaxReference, SyntaxSet};
use syntect_assets::assets::HighlightingAssets;
use crate::mutations::{Conflict, DetectionStatus, Mutation, Mutations, Range};
use crate::{split_lines};

pub enum SysDiffType {
    Simple, // shows the line diff with simple double highlights for the entire mutation region
    Advanced, // shows the line diff with word diff highlights
}

pub enum DiffType {
    New,
    Old,
    Unchanged,
}

impl DiffType {
    pub fn from_change_tag(change_tag: ChangeTag) -> DiffType {
        match change_tag {
            ChangeTag::Delete => DiffType::Old,
            ChangeTag::Insert => DiffType::New,
            ChangeTag::Equal => DiffType::Unchanged,
        }
    }
}

struct Line {
    diff_type: DiffType,
    blocks: Vec<LineBlock>,
    number: usize,
}

struct LineBlock {
    text: String,
    diff_type: DiffType,
}

pub struct Renderer {
    /// represents all mutations known to the renderer
    mutations: Mutations,
    /// represents the cached mutation renders. accessible by indexing with the mutation id.
    mutations_cache: Vec<String>,
    /// represents all mutated source files in map of file path (as in mutations) to file lines.
    source_files: HashMap<PathBuf, Vec<String>>,
    /// stores the current render.
    current_render: String,
    syntax_set: SyntaxSet,
    syntax_ref: SyntaxReference,
    theme: Theme,
}

impl Renderer {
    pub fn new(mutations: Mutations, source_files: HashMap<PathBuf, Vec<String>>) -> Renderer {
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let syntax_ref = syntax_set.find_syntax_by_extension("rs").unwrap().clone();
        let mutations_cache = mutations.values()
            .flat_map(|cs| cs.iter())
            .flat_map(|c| &c.mutations)
            .map(|_| "".to_owned()).collect::<Vec<String>>();
        Renderer {
            mutations,
            mutations_cache,
            source_files,
            current_render: String::new(),
            syntax_set,
            syntax_ref,
            theme: HighlightingAssets::from_binary().get_theme("Monokai Extended Origin").clone(), // TODO: either choose correct theme or make custom one
        }
    }

    fn calc_char_offset(range: &Range, conflict: &Conflict, lines: &Vec<String>) -> usize {
        let lines_offset = range.line - conflict.start_line;
        let mut char_offset = lines_offset;
        for i in 0..lines_offset {
            char_offset += lines[i].len();
        }
        char_offset + range.char
    }

    fn replace(source: &String, replacement: &String, start_index: usize, end_index: usize) -> String {
        String::from(&source[0..start_index]) + replacement + &source[end_index..source.len()]
    }

    // generates the opening <tr> tag for a table row and adds the appropriate row diff class
    // if necessary. also adds the empty old and new line numbers columns. due to line numbers
    // being dynamic when mutations are swapped into the main view, all line numbers will be
    // populated dynamically by javascript.
    fn get_tr_open(line: &mut String, line_type: &DiffType, detection_status: &DetectionStatus, line_number: usize) {
        line.push_str("<tr class=\"line-wrapper");
        match line_type {
            DiffType::Old => line.push_str(" delete"),
            DiffType::New => line.push_str(" insert"),
            DiffType::Unchanged => {}, // unchanged lines need no row diff highlight class
        }
        line.push_str("\"><td class=\"detection-status\"><div class=\"detection-status-marker ");
        match detection_status {
            DetectionStatus::Detected => line.push_str("detected"),
            DetectionStatus::Undetected => line.push_str("undetected"),
            DetectionStatus::Crashed => line.push_str("crashed"),
            DetectionStatus::Timeout => line.push_str("timeout"),
        }
        line.push_str("\"></div></td><td class=\"numbers\">");
        if line_number != 0 {
            line.push_str(&line_number.to_string());
        } else {
            line.push_str("--");
        }
        line.push_str("</td>");
    }

    fn get_tr_close(line: &mut String) {
        line.push_str("</tr>");
    }

    fn highlight(style: Style, text: &str, html_out: &mut String) {
        let rgb = style.foreground;
        html_out.push_str("<span style=\"color: rgb(");
        html_out.push_str(&rgb.r.to_string());
        html_out.push(',');
        html_out.push_str(&rgb.g.to_string());
        html_out.push(',');
        html_out.push_str(&rgb.b.to_string());
        html_out.push_str(")\">");
        html_out.push_str(text);
        html_out.push_str("</span>");
    }

    fn highlight_block(&self, line_block: &LineBlock, html_out: &mut String, highlighter: &mut HighlightLines) {
        match line_block.diff_type {
            DiffType::New => html_out.push_str("<span class=\"inline-diff new\">"),
            DiffType::Old => html_out.push_str("<span class=\"inline-diff old\">"),
            DiffType::Unchanged => {}
        }

        let ranges: Vec<(Style, &str)> = highlighter.highlight_line(&line_block.text, &self.syntax_set).unwrap();
        for (style, text) in ranges {
            Self::highlight(style, text, html_out);
        }

        match line_block.diff_type {
            DiffType::New | DiffType::Old => html_out.push_str("</span>"),
            DiffType::Unchanged => {}
        }
    }

    fn render_mutation(&self, lines: Vec<Line>, html_out: &mut String) {
        let mut highlighter = HighlightLines::new(&self.syntax_ref, &self.theme);

        for line in lines {
            Self::get_tr_open(html_out, &line.diff_type, &DetectionStatus::Undetected, line.number);

            html_out.push_str("<td class=\"line-content\">");
            for line_block in line.blocks {
                self.highlight_block(&line_block, html_out, &mut highlighter);
            }
            html_out.push_str("</td>");

            Self::get_tr_close(html_out);
        }
    }

    pub fn cache_mutations(&mut self, diff_type: SysDiffType) {
        for (path, conflicts) in &self.mutations {
            for conflict in conflicts {
                let conflict_target_lines = self.source_files.get(path).unwrap()[conflict.start_line..=conflict.end_line].to_owned();
                let conflict_target = conflict_target_lines.join("\n");

                for mutation in &conflict.mutations {
                    // calculate the position of the mutation within the conflict region
                    let start_char_index = Self::calc_char_offset(&mutation.starts, conflict, &conflict_target_lines);
                    let end_char_index = Self::calc_char_offset(&mutation.ends, conflict, &conflict_target_lines);

                    // replace the original content with the mutated content
                    let replaced = Self::replace(&conflict_target, &mutation.replacement, start_char_index, end_char_index);

                    // diffing and highlighting
                    let mut mutation_string = String::new();

                    match diff_type {
                        SysDiffType::Simple => {
                            let mut lines: Vec<Line> = Vec::new();
                            let unchanged_start_lines = mutation.starts.line - conflict.start_line;
                            let unchanged_end_lines = conflict.end_line - mutation.ends.line;
                            let mut mutation_end_line_index = conflict_target_lines.len() - unchanged_end_lines - 1;
                            let replaced_lines = split_lines(&replaced);

                            for i in 0..unchanged_start_lines {
                                lines.push(Line {
                                    diff_type: DiffType::Unchanged,
                                    number: conflict.start_line + i,
                                    blocks: vec![
                                        LineBlock {
                                            diff_type: DiffType::Unchanged,
                                            text: conflict_target_lines.get(i).unwrap().to_owned(),
                                        }
                                    ]
                                })
                            }

                            if mutation.starts.line == mutation.ends.line {
                                let old_line = conflict_target_lines.get(unchanged_start_lines).unwrap();
                                lines.push(Line {
                                    diff_type: DiffType::Old,
                                    number: mutation.starts.line,
                                    blocks: vec![
                                        LineBlock {
                                            diff_type: DiffType::Unchanged,
                                            text: old_line[0..mutation.starts.char].to_string(),
                                        },
                                        LineBlock {
                                            diff_type: DiffType::Old,
                                            text: old_line[mutation.starts.char..mutation.ends.char].to_string(),
                                        },
                                        LineBlock {
                                            diff_type: DiffType::Unchanged,
                                            text: old_line[mutation.ends.char..old_line.len()].to_string(),
                                        }
                                    ]
                                });
                            } else {
                                for i in unchanged_start_lines..=mutation_end_line_index {
                                    let line = conflict_target_lines.get(i).unwrap();
                                    let line_number = mutation.starts.line + i - unchanged_start_lines;
                                    if i == unchanged_start_lines {
                                        lines.push(Line {
                                            diff_type: DiffType::Old,
                                            number: line_number,
                                            blocks: vec![
                                                LineBlock {
                                                    diff_type: DiffType::Unchanged,
                                                    text: line[0..mutation.starts.char].to_string(),
                                                },
                                                LineBlock {
                                                    diff_type: DiffType::Old,
                                                    text: line[mutation.starts.char..line.len()].to_string(),
                                                }
                                            ]
                                        })
                                    } else if i == mutation_end_line_index {
                                        lines.push(Line {
                                            diff_type: DiffType::Old,
                                            number: line_number,
                                            blocks: vec![
                                                LineBlock {
                                                    diff_type: DiffType::Old,
                                                    text: line[0..mutation.ends.char].to_string(),
                                                },
                                                LineBlock {
                                                    diff_type: DiffType::Unchanged,
                                                    text: line[mutation.ends.char..line.len()].to_string(),
                                                }
                                            ]
                                        })
                                    } else {
                                        lines.push(Line {
                                            diff_type: DiffType::Old,
                                            number: line_number,
                                            blocks: vec![
                                                LineBlock {
                                                    diff_type: DiffType::Old,
                                                    text: line.to_string(),
                                                }
                                            ]
                                        })
                                    }
                                }
                            }

                            // replaced end line index for replaced_lines
                            mutation_end_line_index = replaced_lines.len() - unchanged_end_lines - 1;

                            if mutation.starts.line == mutation.ends.line && conflict_target_lines.len() == replaced_lines.len() {
                                let new_line = replaced_lines.get(unchanged_start_lines).unwrap();
                                let mutation_end_offset = mutation.starts.char + mutation.replacement.len();
                                lines.push(Line {
                                    diff_type: DiffType::New,
                                    number: 0, // as line numbers cannot be 0, 0 is used as null
                                    blocks: vec![
                                        LineBlock {
                                            diff_type: DiffType::Unchanged,
                                            text: new_line[0..mutation.starts.char].to_string(),
                                        },
                                        LineBlock {
                                            diff_type: DiffType::New,
                                            text: new_line[mutation.starts.char..mutation_end_offset].to_string(),
                                        },
                                        LineBlock {
                                            diff_type: DiffType::Unchanged,
                                            text: new_line[mutation_end_offset..new_line.len()].to_string(),
                                        }
                                    ]
                                })
                            } else {
                                for i in unchanged_start_lines..=mutation_end_line_index {
                                    let line = replaced_lines.get(i).unwrap();
                                    if i == unchanged_start_lines {
                                        lines.push(Line {
                                            diff_type: DiffType::New,
                                            number: 0,
                                            blocks: vec![
                                                LineBlock {
                                                    diff_type: DiffType::Unchanged,
                                                    text: line[0..mutation.starts.char].to_string(),
                                                },
                                                LineBlock {
                                                    diff_type: DiffType::New,
                                                    text: line[mutation.starts.char..line.len()].to_string(),
                                                }
                                            ]
                                        })
                                    } else if i == mutation_end_line_index {
                                        let end_index = split_lines(&mutation.replacement).last().unwrap().len();
                                        lines.push(Line {
                                            diff_type: DiffType::New,
                                            number: 0,
                                            blocks: vec![
                                                LineBlock {
                                                    diff_type: DiffType::New,
                                                    text: line[0..end_index].to_string(),
                                                },
                                                LineBlock {
                                                    diff_type: DiffType::Unchanged,
                                                    text: line[end_index..line.len()].to_string(),
                                                }
                                            ]
                                        })
                                    } else {
                                        lines.push(Line {
                                            diff_type: DiffType::New,
                                            number: 0,
                                            blocks: vec![
                                                LineBlock {
                                                    diff_type: DiffType::New,
                                                    text: line.to_string(),
                                                }
                                            ]
                                        })
                                    }
                                }
                            }

                            mutation_end_line_index = conflict_target_lines.len() - unchanged_end_lines - 1;

                            for i in mutation_end_line_index + 1..conflict_target_lines.len() {
                                lines.push(Line {
                                    diff_type: DiffType::Unchanged,
                                    number: conflict.start_line + i,
                                    blocks: vec![
                                        LineBlock {
                                            diff_type: DiffType::Unchanged,
                                            text: conflict_target_lines.get(i).unwrap().to_owned(),
                                        }
                                    ]
                                })
                            }

                            self.render_mutation(lines, &mut mutation_string);
                        },
                        SysDiffType::Advanced => {
                            let diff = TextDiff::from_lines(&conflict_target, &replaced);
                            let changes: Vec<_> = diff.iter_all_changes().collect();

                            // TODO: need some kind of structure to store line diff indexes and then pass
                            //  all of that into the double_diff_highlighter fn or whatever ill call it...
                            // TODO: is that even the best way of representing it?
                            for (i, change) in changes.iter().enumerate() {
                                match change.tag() {
                                    ChangeTag::Delete => {
                                        // Find corresponding insert
                                        if let Some(insert_change) = changes.get(i + 1)
                                            .filter(|c| c.tag() == ChangeTag::Insert) {

                                            let char_diff = TextDiff::from_words(change.value(), insert_change.value());

                                            let mut output = String::from("-");

                                            for char_change in char_diff.iter_all_changes() {
                                                match char_change.tag() {
                                                    ChangeTag::Delete => {
                                                        output.push_str(&format!("\x1b[31m{}\x1b[0m", char_change.value())); // Red for deleted
                                                    }
                                                    ChangeTag::Insert => {}
                                                    ChangeTag::Equal => {
                                                        output.push_str(char_change.value());
                                                    }
                                                }
                                            }
                                            if !output.ends_with("\n") {
                                                output.push_str("\n");
                                            }
                                            print!("{}", output);
                                        } else {
                                            print!("-\x1b[31m{}\x1b[0m", change.value());
                                            if !change.value().ends_with("\n") {
                                                println!();
                                            }
                                        }
                                    }
                                    ChangeTag::Insert => {
                                        // Find corresponding delete (if needed)
                                        if let Some(delete_change) = changes.get(i - 1)
                                            .filter(|c| c.tag() == ChangeTag::Delete) {

                                            let char_diff = TextDiff::from_words(delete_change.value(), change.value());

                                            print!("+");

                                            for change in char_diff.iter_all_changes() {
                                                match change.tag() {
                                                    ChangeTag::Delete => {}
                                                    ChangeTag::Insert => {
                                                        print!("\x1b[32m{}\x1b[0m", change.value()); // Green for inserted
                                                    }
                                                    ChangeTag::Equal => {
                                                        print!("{}", change.value());
                                                    }
                                                }
                                            }
                                        } else {
                                            print!("+\x1b[32m{}\x1b[0m", change.value());
                                        }
                                    }
                                    ChangeTag::Equal => {
                                        print!("={}", change.value());
                                    } // Ignore Equal changes
                                }
                            }
                        },
                    }

                    self.mutations_cache[mutation.mutation_id] = mutation_string;
                }
            }
        }
    }

    pub fn render_file(&mut self, path: PathBuf) -> &String {
        self.current_render = String::new();
        // TODO: follow the process through to render one entire file
        //  process: render file tree
        //  process: render code view title
        //  process: render code view with diff
        &self.current_render
    }

    fn render_file_tree(&mut self, file: PathBuf) {
        // TODO: render the file tree based off of the
    }

    fn render_source_code(&mut self, file: PathBuf) {
        // TODO: go through each line and render the source code. also deal with inserting
        //  mutations into this. should use an increment in the for loop so that lines can be
        //  skipped.
    }
}