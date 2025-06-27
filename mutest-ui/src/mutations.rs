use std::collections::HashMap;
use std::path::PathBuf;
use mutest_json::Idx;
use crate::mutations::DetectionStatus::{Crashed, Detected, Timeout, Undetected};

/// Represents the detection status of a mutation.
pub(crate) enum DetectionStatus {
    Detected,   // mutest: 'D'
    Undetected, // mutest: '-'
    Crashed,    // mutest: 'C'
    Timeout,    // mutest: 'T'
}

impl DetectionStatus {
    pub fn from(s: u8) -> Option<Self> {
        match s as char {
            'D' => Some(Detected),
            '-' => Some(Undetected),
            'C' => Some(Crashed),
            'T' => Some(Timeout),
            _ => None,
        }
    }
}

/// Represents a line and character representing either the start or end of a mutated region.
pub(crate) struct Range {
    pub line: usize,
    pub char: usize,
}

impl Range {
    pub fn new((line, char): (usize, usize)) -> Self {
        Self { line, char }
    }
}

/// A streamlined mutation datatype that contains only the data needed by mutest-ui. It combines
/// data from several of the raw mutest output files into a single struct.
pub(crate) struct Mutation {
    pub mutation_id: usize,
    pub mutation_op: String,
    pub name: String,
    pub starts: Range,
    pub ends: Range,
    pub replacement: String,
    pub detection_status: Option<DetectionStatus>,
}

impl Mutation {
    pub fn new(mutation: mutest_json::mutations::Mutation, evaluation: &mutest_json::evaluation::EvaluationInfo) -> Self {
        Mutation {
            mutation_id: mutation.mutation_id.as_index(),
            mutation_op: mutation.mutation_op,
            name: mutation.display_name,
            starts: Range::new(mutation.origin_span.begin),
            ends: Range::new(mutation.origin_span.end),
            replacement: mutation.substs.into_iter().next().unwrap().substitute.replacement,
            detection_status: DetectionStatus::from(evaluation.mutation_runs.first().unwrap()
                .mutation_detection_matrix.overall_detections.as_bytes()[mutation.mutation_id.as_index()])
        }
    }
}

/// Represents a region of lines where several grouped mutations would conflict with each other if
/// displayed inline simultaneously
pub(crate) struct Conflict {
    pub start_line: usize,
    pub end_line: usize,
    pub mutations: Vec<Mutation>,
}

impl Conflict {
    fn new(mutation: Mutation) -> Self {
        Conflict {
            start_line: mutation.starts.line,
            end_line: mutation.ends.line,
            mutations: vec![mutation],
        }
    }
    
    fn conflicts(&self, mutation: &Mutation) -> bool {
        mutation.starts.line >= self.start_line && mutation.starts.line <= self.end_line ||
            mutation.ends.line >= self.start_line && mutation.ends.line <= self.end_line
    }

    fn update(&mut self, mutation: Mutation) {
        if mutation.ends.line > self.end_line {
            self.end_line = mutation.ends.line
        }
        self.mutations.push(mutation);
    }
}

/// Represents all the mutations in for a given file. Each mutation is stored in a conflict, so
/// that conflicting mutations can be correctly displayed in the interface.
pub(crate) type Mutations = HashMap<PathBuf, Vec<Conflict>>;

/// Converts the raw mutest output into a streamlined format that mutest-ui uses.
pub(crate) fn streamline_mutations(metadata: crate::Metadata) -> Mutations {
    let mut mutations = Mutations::new();

    'mutations: for mutation in metadata.mutations.mutations {
        let path = mutation.origin_span.path.clone();
        let mut file = mutations.entry(path).or_default();
        let streamlined = Mutation::new(mutation, &metadata.evaluation);
        
        for conflict in file.iter_mut() {
            if conflict.conflicts(&streamlined) {
                conflict.update(streamlined);
                continue 'mutations;
            }
        }
        
        file.push(Conflict::new(streamlined));
    }

    mutations
}
