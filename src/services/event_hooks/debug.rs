/*
    A implementation of EventHooks that prints to stdout for debugging purposes.
*/

use crate::imports::*;

pub struct DebugEventHooks;

#[async_trait::async_trait]
impl EventHooks for DebugEventHooks {
    // Draft phase
    async fn after_added_namespace(&self, event: &hook_events::NamespaceInit) -> Result<()> {
        println!("DebugEventHooks - after_added_namespace: {:?}", event);
        Ok(())
    }
    async fn after_added_command(&self, event: &hook_events::CommandInit) -> Result<()> {
        println!("DebugEventHooks - after_added_command: {:?}", event);
        Ok(())
    }
    async fn before_compile_pipeline(&self, event: &hook_events::PipelineInfo) -> Result<()> {
        println!("DebugEventHooks - before_compile_pipeline: {:?}", event);
        Ok(())
    }
    async fn after_compile_pipeline(&self, event: &hook_events::PipelineCompiled) -> Result<()> {
        println!("DebugEventHooks - after_compile_pipeline: {:?}", event);
        Ok(())
    }

    // Ready phase
    async fn before_execute_pipeline(&self, event: &hook_events::PipelineInfo) -> Result<()> {
        println!("DebugEventHooks - before_execute_pipeline: {:?}", event);
        Ok(())
    }
    async fn after_execute_pipeline(&self, event: &hook_events::PipelineExecuted) -> Result<()> {
        println!("DebugEventHooks - after_execute_pipeline: {:?}", event);
        Ok(())
    }
    async fn before_execute_namespace(&self, event: &hook_events::NamespaceInfo) -> Result<()> {
        println!("DebugEventHooks - before_execute_namespace: {:?}", event);
        Ok(())
    }
    async fn after_execute_namespace(&self, event: &hook_events::NamespaceExecuted) -> Result<()> {
        println!("DebugEventHooks - after_execute_namespace: {:?}", event);
        Ok(())
    }
    async fn before_execute_command(&self, event: &hook_events::CommandInfo) -> Result<()> {
        println!("DebugEventHooks - before_execute_command: {:?}", event);
        Ok(())
    }
    async fn after_execute_command(&self, event: &hook_events::CommandExecuted) -> Result<()> {
        println!("DebugEventHooks - after_execute_command: {:?}", event);
        Ok(())
    }

    // Completed phase
    async fn on_results_start(&self, event: &hook_events::PipelineInfo) -> Result<()> {
        println!("DebugEventHooks - on_results_start: {:?}", event);
        Ok(())
    }
    async fn on_results_finish(&self, event: &hook_events::PipelineCompleted) -> Result<()> {
        println!("DebugEventHooks - on_results_finish: {:?}", event);
        Ok(())
    }
}
