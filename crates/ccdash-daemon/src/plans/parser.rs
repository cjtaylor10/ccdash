//! Parses markdown plan files into structured Phase/Task records.
//!
//! Convention (matches superpowers:writing-plans output):
//! - `## Phase N: Title` or `## Task N: Title` → a phase boundary
//! - GitHub-flavored task list items (`- [ ]` / `- [x]`) → tasks
//! - First `# Heading` of the file → plan title (falls back to filename stem)

use ccdash_core::protocol::{Plan, PlanPhase, PlanTask};
use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use std::path::Path;

pub fn parse(path: &Path, text: &str) -> Plan {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_TASKLISTS);
    let parser = Parser::new_ext(text, options);

    let mut title: Option<String> = None;
    let mut phases: Vec<PlanPhase> = Vec::new();
    let mut current_text = String::new();
    let mut in_heading_level: Option<HeadingLevel> = None;
    let mut in_list_item = false;
    let mut current_task_done: Option<bool> = None;
    let mut current_task_text = String::new();

    for evt in parser {
        match evt {
            Event::Start(Tag::Heading { level, .. }) => {
                in_heading_level = Some(level);
                current_text.clear();
            }
            Event::End(TagEnd::Heading(level)) => {
                let text = current_text.trim().to_string();
                in_heading_level = None;
                current_text.clear();
                if level == HeadingLevel::H1 && title.is_none() {
                    title = Some(text);
                } else if level == HeadingLevel::H2 || level == HeadingLevel::H3 {
                    if text.starts_with("Phase ")
                        || text.starts_with("Task ")
                        || text.starts_with("Section ")
                    {
                        phases.push(PlanPhase {
                            name: text,
                            tasks: Vec::new(),
                        });
                    }
                }
            }
            Event::Start(Tag::Item) => {
                in_list_item = true;
                current_task_done = None;
                current_task_text.clear();
            }
            Event::End(TagEnd::Item) => {
                if in_list_item {
                    if let Some(done) = current_task_done {
                        let title = current_task_text.trim().to_string();
                        if let Some(phase) = phases.last_mut() {
                            phase.tasks.push(PlanTask { title, done });
                        } else {
                            phases.push(PlanPhase {
                                name: "(top level)".into(),
                                tasks: vec![PlanTask { title, done }],
                            });
                        }
                    }
                }
                in_list_item = false;
                current_task_done = None;
                current_task_text.clear();
            }
            Event::TaskListMarker(done) => {
                current_task_done = Some(done);
            }
            Event::Text(t) => {
                if in_heading_level.is_some() {
                    current_text.push_str(&t);
                } else if in_list_item && current_task_done.is_some() {
                    current_task_text.push_str(&t);
                }
            }
            Event::Code(t) => {
                if in_heading_level.is_some() {
                    current_text.push_str(&t);
                } else if in_list_item && current_task_done.is_some() {
                    current_task_text.push_str(&t);
                }
            }
            _ => {}
        }
    }

    let title = title.unwrap_or_else(|| {
        path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("plan")
            .to_string()
    });

    Plan {
        path: path.to_path_buf(),
        title,
        phases,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_title_and_phases() {
        let md = "\
# My Plan

## Phase 1: Setup

- [ ] do thing
- [x] did thing

## Phase 2: Run

- [ ] launch
";
        let p = parse(Path::new("/tmp/x.md"), md);
        assert_eq!(p.title, "My Plan");
        assert_eq!(p.phases.len(), 2);
        assert_eq!(p.phases[0].name, "Phase 1: Setup");
        assert_eq!(p.phases[0].tasks.len(), 2);
        assert!(!p.phases[0].tasks[0].done);
        assert!(p.phases[0].tasks[1].done);
        assert_eq!(p.phases[1].tasks.len(), 1);
    }

    #[test]
    fn tasks_before_any_phase_go_under_top_level() {
        let md = "- [ ] orphan task\n";
        let p = parse(Path::new("/tmp/x.md"), md);
        assert_eq!(p.phases.len(), 1);
        assert_eq!(p.phases[0].name, "(top level)");
        assert_eq!(p.phases[0].tasks.len(), 1);
    }

    #[test]
    fn falls_back_to_filename_title() {
        let p = parse(Path::new("/tmp/cool-thing.md"), "## Phase 1: foo\n- [ ] x\n");
        assert_eq!(p.title, "cool-thing");
    }

    #[test]
    fn ignores_non_task_list_items() {
        let md = "## Phase 1: foo\n- regular item\n- [ ] real task\n";
        let p = parse(Path::new("/tmp/x.md"), md);
        assert_eq!(p.phases[0].tasks.len(), 1);
        assert_eq!(p.phases[0].tasks[0].title, "real task");
    }

    #[test]
    fn h3_task_heading_is_a_phase() {
        let md = "### Task A1: Init\n- [ ] step\n";
        let p = parse(Path::new("/tmp/x.md"), md);
        assert_eq!(p.phases.len(), 1);
        assert_eq!(p.phases[0].name, "Task A1: Init");
    }
}
