use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::thread::current;
use std::time::Instant;
use similar::{Algorithm, ChangeTag, TextDiff, TextDiffConfig};
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style, Theme, ThemeSet};
use syntect::parsing::{SyntaxReference, SyntaxSet};
use uuid::Uuid;
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
    syntax_set: SyntaxSet,
    syntax_ref: SyntaxReference,
    theme: Theme,
    no_lines_rendered: usize,
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
            syntax_set,
            syntax_ref,
            theme: ThemeSet::load_from_folder("mutest-ui/src/assets/themes").unwrap().themes["Darcula"].clone(),
            no_lines_rendered: 0,
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

    fn get_detection_status_marker(line: &mut String, detection_status: &Option<DetectionStatus>) {
        let status: &str = match detection_status {
            None => "",
            Some(DetectionStatus::Detected) => " detected",
            Some(DetectionStatus::Undetected) => " undetected",
            Some(DetectionStatus::Crashed) => " crashed",
            Some(DetectionStatus::Timeout) => " timeout",
        };
        line.push_str("<div class=\"detection-status-marker");
        line.push_str(status);
        line.push_str("\">");
        line.push_str(status);
        line.push_str("</div>");
    }

    // generates the opening <tr> tag for a table row and adds the appropriate row diff class
    // if necessary. also adds the empty old and new line numbers columns. due to line numbers
    // being dynamic when mutations are swapped into the main view, all line numbers will be
    // populated dynamically by javascript.
    fn get_tr_open(line: &mut String, line_type: &DiffType, detection_status: &Option<DetectionStatus>, line_number: usize) {
        line.push_str("<tr class=\"line-wrapper");
        match line_type {
            DiffType::Old => line.push_str(" delete"),
            DiffType::New => line.push_str(" insert"),
            DiffType::Unchanged => {}, // unchanged lines need no row diff highlight class
        }
        line.push_str("\"><td class=\"detection-status");
        if line_number == 0 {
            line.push_str(" new")
        }
        line.push_str("\">");
        Self::get_detection_status_marker(line, detection_status);
        line.push_str("</td><td class=\"numbers");
        if line_number != 0 {
            line.push_str("\">");
            line.push_str(&line_number.to_string());
        } else {
            line.push_str(" new\">");
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
        html_out.push_str(html_escape::encode_text(text).as_ref());
        html_out.push_str("</span>");
    }

    fn highlight_block(&self, line_block: &LineBlock, html_out: &mut String, highlighter: &mut HighlightLines) {
        match line_block.diff_type {
            DiffType::New => html_out.push_str("<span class=\"inline-diff new\">"),
            DiffType::Old => html_out.push_str("<span class=\"inline-diff old\">"),
            DiffType::Unchanged => {}
        }

        self.highlight_line(&line_block.text, html_out, highlighter);

        match line_block.diff_type {
            DiffType::New | DiffType::Old => html_out.push_str("</span>"),
            DiffType::Unchanged => {}
        }
    }

    fn highlight_line(&self, line: &str, html_out: &mut String, highlighter: &mut HighlightLines) {
        let ranges: Vec<(Style, &str)> = highlighter.highlight_line(&line, &self.syntax_set).unwrap();
        for (style, text) in ranges {
            Self::highlight(style, text, html_out);
        }
    }

    fn render_mutation(&self, lines: Vec<Line>, mutation: &Mutation, html_out: &mut String) {
        let mut highlighter = HighlightLines::new(&self.syntax_ref, &self.theme);

        let mut is_first_line = true;
        for line in lines {
            if is_first_line {
                Self::get_tr_open(html_out, &line.diff_type, &mutation.detection_status, line.number);
                is_first_line = false;
            } else {
                Self::get_tr_open(html_out, &line.diff_type, &None, line.number);
            }

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
                    let mut lines: Vec<Line> = Vec::new();
                    let mut mutation_string = String::new();

                    match diff_type {
                        SysDiffType::Simple => {
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
                                            text: conflict_target_lines.get(i).unwrap().to_string(),
                                        }
                                    ]
                                });
                                self.no_lines_rendered += 1;
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
                                self.no_lines_rendered += 3;
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
                                        });
                                        self.no_lines_rendered += 2;
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
                                        });
                                        self.no_lines_rendered += 2;
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
                                        });
                                        self.no_lines_rendered += 1;
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
                                });
                                self.no_lines_rendered += 3;
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
                                        });
                                        self.no_lines_rendered += 2;
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
                                        });
                                        self.no_lines_rendered += 2;
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
                                        });
                                        self.no_lines_rendered += 1;
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
                                            text: conflict_target_lines.get(i).unwrap().to_string(),
                                        }
                                    ]
                                });
                                self.no_lines_rendered += 1;
                            }
                        },
                        SysDiffType::Advanced => {
                            let diff = TextDiff::from_lines(&conflict_target, &replaced);
                            let changes: Vec<_> = diff.iter_all_changes().collect();

                            for (i, change) in changes.iter().enumerate() {
                                match change.tag() {
                                    ChangeTag::Delete => {
                                        // Find corresponding insert
                                        if let Some(insert_change) = changes.get(i + 1)
                                            .filter(|c| c.tag() == ChangeTag::Insert) {

                                            let word_diff = TextDiff::from_words(change.value(), insert_change.value());
                                            let mut line = Line {
                                                diff_type: DiffType::Old,
                                                number: conflict.start_line + i,
                                                blocks: vec![],
                                            };

                                            let mut inline_delete_value = String::new();
                                            let mut inline_unchange_value = String::new();

                                            for word_change in word_diff.iter_all_changes() {
                                                match word_change.tag() {
                                                    ChangeTag::Delete => {
                                                        inline_delete_value.push_str(word_change.value());

                                                        if inline_unchange_value != "" {
                                                            line.blocks.push(LineBlock {
                                                                diff_type: DiffType::Unchanged,
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
                                                                diff_type: DiffType::Old,
                                                                text: inline_delete_value.clone(),
                                                            });
                                                            inline_delete_value = String::new();
                                                        }
                                                    }
                                                }
                                            }

                                            if inline_unchange_value != "" {
                                                line.blocks.push(LineBlock {
                                                    diff_type: DiffType::Unchanged,
                                                    text: inline_unchange_value.clone(),
                                                });
                                            }

                                            if inline_delete_value != "" {
                                                line.blocks.push(LineBlock {
                                                    diff_type: DiffType::Old,
                                                    text: inline_delete_value.clone(),
                                                });
                                            }

                                            lines.push(line);
                                        } else {
                                            lines.push(Line {
                                                diff_type: DiffType::Old,
                                                number: conflict.start_line + i,
                                                blocks: vec![
                                                    LineBlock {
                                                        diff_type: DiffType::Unchanged,
                                                        text: change.value().to_string(),
                                                    }
                                                ]
                                            });
                                        }
                                    }
                                    ChangeTag::Insert => {
                                        // Find corresponding delete (if needed)
                                        if let Some(delete_change) = changes.get(i - 1)
                                            .filter(|c| c.tag() == ChangeTag::Delete) {

                                            let word_diff = TextDiff::from_words(delete_change.value(), change.value());
                                            let mut line = Line {
                                                diff_type: DiffType::New,
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
                                                                diff_type: DiffType::Unchanged,
                                                                text: inline_unchange_value.clone(),
                                                            });
                                                            inline_unchange_value = String::new();
                                                        }
                                                    }
                                                    ChangeTag::Equal => {
                                                        inline_unchange_value.push_str(word_change.value());

                                                        if inline_insert_value != "" {
                                                            line.blocks.push(LineBlock {
                                                                diff_type: DiffType::New,
                                                                text: inline_insert_value.clone(),
                                                            });
                                                            inline_insert_value = String::new();
                                                        }
                                                    }
                                                }
                                            }

                                            if inline_unchange_value != "" {
                                                line.blocks.push(LineBlock {
                                                    diff_type: DiffType::Unchanged,
                                                    text: inline_unchange_value.clone(),
                                                });
                                            }

                                            if inline_insert_value != "" {
                                                line.blocks.push(LineBlock {
                                                    diff_type: DiffType::New,
                                                    text: inline_insert_value.clone(),
                                                });
                                            }

                                            lines.push(line);
                                        } else {
                                            lines.push(Line {
                                                diff_type: DiffType::New,
                                                number: 0,
                                                blocks: vec![
                                                    LineBlock {
                                                        diff_type: DiffType::Unchanged,
                                                        text: change.value().to_string(),
                                                    }
                                                ]
                                            });
                                        }
                                    }
                                    ChangeTag::Equal => {
                                        lines.push(Line {
                                            diff_type: DiffType::Unchanged,
                                            number: conflict.start_line + i,
                                            blocks: vec![
                                                LineBlock {
                                                    diff_type: DiffType::Unchanged,
                                                    text: change.value().to_string(),
                                                }
                                            ]
                                        });
                                    }
                                }
                            }
                        },
                    }

                    self.render_mutation(lines, &mutation, &mut mutation_string);
                    self.mutations_cache[mutation.mutation_id] = mutation_string;
                }
            }
        }
    }

    pub fn render_file(&mut self, path: &PathBuf, path_depth: usize, styles: &Vec<PathBuf>, scripts: &Vec<PathBuf>) -> String {
        let mut current_render = String::from("<!DOCTYPE html><html><head>");
        let mut path_prefix = String::with_capacity(64);

        for _ in 0..path_depth {
            path_prefix.push_str("../");
        }

        for style in styles {
            current_render.push_str(&format!("<link rel=\"stylesheet\" href=\"{}{}\" />", path_prefix, style.display()));
        }

        for script in scripts {
            current_render.push_str(&format!("<script type=\"text/javascript\" src=\"{}{}\"></script>", path_prefix, script.display()));
        }

        current_render.push_str("</head><body>");
        self.render_source_code(path, &mut current_render);
        current_render.push_str("</body></html>");
        current_render
    }

    fn render_file_tree(&mut self, path: &PathBuf) {
        // TODO: render the file tree based off of the
    }

    fn render_source_code(&mut self, path: &PathBuf, html_out: &mut String) {
        let file_lines = self.source_files.get(path).unwrap();
        let mut file_conflicts = &self.mutations.get(path).unwrap()[..];
        let mut highlighter = HighlightLines::new(&self.syntax_ref, &self.theme);
        let mut mutation_changer = String::from("<div id=\"changer\" class=\"mutation-changer hidden\">");
        let standard_columns = String::from("<colgroup><col span=\"1\" style=\"width: 80px;\"><col span=\"1\" style=\"width: 50px;\"><col span=\"1\" style=\"width: auto;\"></colgroup>");
        let changer_columns = String::from("<colgroup><col span=\"1\" style=\"width: 50px;\"><col span=\"1\" style=\"width: auto;\"></colgroup>");

        html_out.push_str("<div class=\"main-code-wrapper\"><table>");
        html_out.push_str(&standard_columns);
        html_out.push_str("<tbody>");

        let mut file_lines_iter = 0..file_lines.len();
        // for mut i in 0..file_lines.len() {
        while let Some(i) = file_lines_iter.next() {
            if let Some(conflict) = file_conflicts.first() {
                if conflict.start_line == i {
                    for _ in conflict.start_line..conflict.end_line { file_lines_iter.next(); }
                    let section_name = format!("conflict-{}", Uuid::new_v4());
                    html_out.push_str("<tbody class=\"");
                    html_out.push_str(&section_name);
                    if conflict.mutations.len() > 1 {
                        html_out.push_str(" mutation-conflict-region");
                    }
                    html_out.push_str("\">");
                    if conflict.mutations.len() > 1 {
                        html_out.push_str("<tr><td colspan=\"3\" class=\"mutation-conflict-header\">1 of ");
                        html_out.push_str(&conflict.mutations.len().to_string());
                        html_out.push_str(" mutations in region [");
                        html_out.push_str(&format!("{}:{}", conflict.start_line, conflict.end_line));
                        html_out.push_str("], Click region to show all mutations</td></tr>");
                    }
                    html_out.push_str(&self.mutations_cache[conflict.mutations.first().unwrap().mutation_id]);
                    html_out.push_str("</tbody>");
                    if conflict.mutations.len() > 1 {
                        let mut i = 2;
                        for mutation in &conflict.mutations[1..] {
                            html_out.push_str("<tbody class=\"");
                            html_out.push_str(&section_name);
                            html_out.push_str(" mutation-conflict-region hidden\">");

                            // TODO: refactor this into a function, identical code used earlier
                            html_out.push_str("<tr><td colspan=\"3\" class=\"mutation-conflict-header\">");
                            html_out.push_str(&i.to_string());
                            html_out.push_str(" of ");
                            html_out.push_str(&conflict.mutations.len().to_string());
                            html_out.push_str(" mutations in region [");
                            html_out.push_str(&format!("{}:{}", conflict.start_line, conflict.end_line));
                            html_out.push_str("], Click region to show all mutations</td></tr>");

                            html_out.push_str(&self.mutations_cache[mutation.mutation_id]);
                            html_out.push_str("</tbody>");

                            i += 1;
                        }

                        // adding mutations to the mutation changer interface
                        mutation_changer.push_str("<div id=\"");
                        mutation_changer.push_str(&section_name);
                        mutation_changer.push_str("\" class=\"mutations\">");
                        for mutation in &conflict.mutations {
                            mutation_changer.push_str("<div class=\"mutation-content-wrapper\">");
                            mutation_changer.push_str(&format!("<h2 class=\"mutation-name\"><span class=\"mutation-id\">{}</span> {}</h2>", mutation.mutation_id, &mutation.name));
                            Self::get_detection_status_marker(&mut mutation_changer, &mutation.detection_status);
                            mutation_changer.push_str("<div class=\"mutation-wrapper\" data-target-class=\"");
                            mutation_changer.push_str(&section_name);
                            mutation_changer.push_str("\"><table class=\"no-status no-line-wrapper\">");
                            mutation_changer.push_str(&changer_columns);
                            mutation_changer.push_str("<tbody>");
                            mutation_changer.push_str(&self.mutations_cache[mutation.mutation_id]);
                            mutation_changer.push_str("</tbody></table></div></div>");
                        }
                        mutation_changer.push_str("</div>");
                    }

                    file_conflicts = &file_conflicts[1..];
                    continue;
                }
            }

            Self::get_tr_open(html_out, &DiffType::Unchanged, &None, i + 1);
            html_out.push_str("<td class=\"line-content\">");
            self.highlight_line(&file_lines[i], html_out, &mut highlighter);
            html_out.push_str("</td>");
            Self::get_tr_close(html_out);
            self.no_lines_rendered += 1;
        }

        mutation_changer.push_str("</div>");
        html_out.push_str("</tbody></table></div>");
        html_out.push_str(&mutation_changer);
    }

    pub fn get_no_lines_rendered(&self) -> usize {
        self.no_lines_rendered
    }
}