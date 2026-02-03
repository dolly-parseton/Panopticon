/*
    Types for supporting Hook trait:
    * NamespaceInit
    * CommandInit
    * PipelineInfo
    * PipelineCompiled
    * PipelineExecuted
    * PipelineCompleted
    * NamespaceInfo
    * NamespaceExecuted
    * CommandInfo
    * CommandExecuted

    These are placeholders for now.
*/
use crate::imports::*;

#[derive(Debug)]
pub struct PipelineInfo {
    pub namespace_count: usize,
    pub command_count: usize,
}

#[derive(Debug)]
pub struct PipelineCompiled {
    pub namespace_count: usize,
    pub command_count: usize,
    pub compiled_at: Instant,
}

#[derive(Debug)]
pub struct PipelineExecuted {
    pub namespace_count: usize,
    pub command_count: usize,
    pub executed_at: Instant,
}

#[derive(Debug)]
pub struct PipelineCompleted {
    pub namespace_count: usize,
    pub command_count: usize,
    pub completed_at: Instant,
}

#[derive(Debug)]
pub struct NamespaceInit {
    pub namespace_index: usize,
    pub namespace_name: String,
    pub namespace_type: String,
}

#[derive(Debug)]
pub struct NamespaceInfo {
    pub namespace_index: usize,
    pub namespace_name: String,
    pub command_count: usize,
}

#[derive(Debug)]
pub struct NamespaceExecuted {
    pub namespace_index: usize,
    pub namespace_name: String,
    pub executed_at: Instant,
}

#[derive(Debug)]
pub struct CommandInit {
    pub namespace_index: usize,
    pub command_name: String,
    pub command_type: String,
}

#[derive(Debug)]
pub struct CommandInfo {
    pub namespace_index: usize,
    pub command_name: String,
    pub command_type: String,
    pub command_count: usize,
}

#[derive(Debug)]
pub struct CommandExecuted {
    pub namespace_index: usize,
    pub command_name: String,
    pub command_type: String,
    pub executed_at: Instant,
}
