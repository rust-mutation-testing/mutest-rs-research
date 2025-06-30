use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use syntect::highlighting::Theme;
use syntect::parsing::{SyntaxReference, SyntaxSet};
use syntect_assets::assets::HighlightingAssets;
use crate::mutations::{Mutation, Mutations};

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
        Renderer {
            mutations,
            mutations_cache: Vec::new(),
            source_files,
            current_render: String::new(),
            syntax_set,
            syntax_ref,
            theme: HighlightingAssets::from_binary().get_theme("Monokai Extended Origin").clone(), // TODO: either choose correct theme or make custom one
        }
    }

    pub fn cache_mutations(&mut self) {
        // TODO: create the map of mutation id to rendered mutation
    }
    
    fn render_mutation(&mut self, mutation: Mutation) {
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