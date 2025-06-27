use std::collections::HashMap;
use std::path::PathBuf;
use mutest_json::Idx;
use crate::mutations::DetectionStatus::{CRASHED, DETECTED, TIMEOUT, UNDETECTED};

/// DetectionStatus represents the detection status of a StreamlinedMutation.
pub(crate) enum DetectionStatus {
    DETECTED,   // mutest: 'D'
    UNDETECTED, // mutest: '-'
    CRASHED,    // mutest: 'C'
    TIMEOUT,    // mutest: 'T'
}

fn get_detection_status(s: &char) -> Option<DetectionStatus> {
    match s {
        'D' => Some(DETECTED),
        '-' => Some(UNDETECTED),
        'C' => Some(CRASHED),
        'T' => Some(TIMEOUT),
        _ => None,
    }
}

/// StreamlinedMutation is a streamlined mutation datatype that contains only the data needed by
/// mutest-ui. This datatype combines data from several of the raw mutest output files.
pub(crate) struct StreamlinedMutation {
    pub mutation_id: usize,
    pub detection_status: Option<DetectionStatus>,
    pub start_line: usize,
    pub end_line: usize,
    pub start_char_index: usize,
    pub end_char_index: usize,
    pub mutation_op: String,
    pub display_name: String,
    pub substitution: String,
}

/// MutationOverlapRegion represents a region of lines where several grouped mutations would
/// overlap with each other if displayed inline simultaneously
pub(crate) struct MutationOverlapRegion {
    pub start_line: usize,
    pub end_line: usize,
    pub mutations: Vec<StreamlinedMutation>,
}

impl MutationOverlapRegion {
    fn overlaps(&self, mutation: &StreamlinedMutation) -> bool {
        mutation.start_line >= self.start_line && mutation.start_line <= self.end_line
    }

    fn update(&mut self, mutation: StreamlinedMutation) {
        if mutation.end_line > self.end_line {
            self.end_line = mutation.end_line
        }
        self.mutations.push(mutation);
    }
}

/// FileMutations stores all the mutations in for a given file (key) in a vector of
/// MutationOverlapRegions.
pub(crate) type FileMutations = HashMap<PathBuf, Vec<MutationOverlapRegion>>;

pub(crate) fn streamline_mutations(metadata: &crate::Metadata) -> FileMutations {
    let mut mutations = FileMutations::new();
    let mut status_matrix = metadata.evaluation.mutation_runs.first().unwrap().mutation_detection_matrix.overall_detections.chars();

    'mutations: for mutation in &metadata.mutations.mutations {
        let subst = mutation.substs.first().unwrap();
        let (sl, sc) = subst.location.span().begin;
        let (el, ec) = subst.location.span().end;
        let id = mutation.mutation_id.as_index();
        let streamlined = StreamlinedMutation {
            mutation_id: id,
            detection_status: get_detection_status(&status_matrix.nth(id).unwrap()),
            start_line: sl,
            end_line: el,
            start_char_index: sc,
            end_char_index: ec,
            mutation_op: mutation.mutation_op.clone(),
            display_name: mutation.display_name.clone(),
            substitution: subst.substitute.replacement.clone(),
        };
        let file = mutations.get_mut(&subst.location.span().path);
        
        if let Some(file) = file {
            for group in file {
                if group.overlaps(&streamlined) {
                    group.update(streamlined);
                    continue 'mutations;
                }
            }
        }
        
        let mut vec: Vec<StreamlinedMutation> = Vec::new();
        vec.push(streamlined);
        let group = MutationOverlapRegion{
            start_line: sl,
            end_line: el,
            mutations: vec,
        };
    }

    mutations
}
