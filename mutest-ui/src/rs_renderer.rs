use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use similar::{Algorithm, ChangeTag, TextDiff, TextDiffConfig};
use syntect::highlighting::{Style, Theme};
use syntect::parsing::{SyntaxReference, SyntaxSet};
use syntect_assets::assets::HighlightingAssets;
use crate::mutations::{Conflict, Mutation, Mutations, Range};
use crate::{split_lines};

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
        let num_mutations = mutations.values()
            .flat_map(|cs| cs.iter())
            .flat_map(|c| &c.mutations).collect::<Vec<_>>().len();
        Renderer {
            mutations,
            mutations_cache: std::iter::repeat_n("".to_owned(), num_mutations).collect(),
            source_files,
            current_render: String::new(),
            syntax_set,
            syntax_ref,
            theme: HighlightingAssets::from_binary().get_theme("Monokai Extended Origin").clone(), // TODO: either choose correct theme or make custom one
        }
    }

    fn calc_char_offset(range: &Range, conflict: &Conflict, lines: &Vec<String>) -> usize {
        let lines_offset = range.line - conflict.start_line;
        let mut char_offset = 0;
        for i in 0..lines_offset {
            char_offset += lines[i].len();
        }
        char_offset + range.char
    }

    fn replace(source: &String, replacement: &String, start_index: usize, end_index: usize) -> String {
        String::from(&source[0..start_index]) + replacement + &source[end_index..source.len()]
    }

    pub fn cache_mutations(&mut self) {
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

                    // TODO: diffing and highlighting
                    let diff = TextDiff::from_lines(&conflict_target, &replaced);
                    let changes: Vec<_> = diff.iter_all_changes().collect();

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

                    println!("\n\nnext mutation-------------------------------------------\n");
                    
                    // TODO: render as html
                    self.mutations_cache.insert(mutation.mutation_id, String::new());
                }
            }
        }
    }

    fn render_mutation(&self) {
        // TODO: take a slice of the lines required, diff them and render that out into a single
        //  string, then insert the string into the cache.
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