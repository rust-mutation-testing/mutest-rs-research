use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::ptr::addr_eq;
use crate::{read_file, split_lines};

pub(crate) type FilesMap = HashMap<PathBuf, Vec<String>>;

pub(crate) struct Files {
    files: FilesMap,
}

impl Files {
    pub fn new(root: &PathBuf, paths: Vec<PathBuf>) -> Result<Self, Box<dyn std::error::Error>> {
        let mut files = HashMap::new();
        for path in paths {
            let file_lines = split_lines(&read_file(&root.join(&path))?);
            files.insert(path, file_lines);
        }
        Ok(Self { files })
    }
    
    pub fn get_files_map(self) -> FilesMap {
        self.files.clone() // TODO: this is not very performant, but it may be needed in multiple places?
    }

    fn read_file(path: &PathBuf) -> Result<String, std::io::Error> {
        Ok(fs::read_to_string(path)?)
    }

    fn split_lines(data: &String) -> Vec<String> {
        data.replace("\r", "").split("\n").map(|s| s.to_string()).collect()
    }
}