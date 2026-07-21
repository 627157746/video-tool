# Design: Managed job groups

## Model

```rust
pub struct JobGroupDefinition {
    pub id: String,
    pub name: String,
}

// AppConfig
#[serde(default)]
pub job_groups: Vec<JobGroupDefinition>,
```

Job.group continues as `Option<String>` but **canonical value is group id**.

## Resolve / ensure

```text
input empty -> None
input matches existing id -> that id
input matches existing name (trim, case-insensitive) -> that id
else -> create { id: uuid, name: trimmed }, append, persist if via config lock
```

Create-job and update_job_group call ensure-with-persist when they introduce a new name.

## Save config cascade

When candidate.job_groups drops ids that current workspace jobs still reference:

1. If any such job is running -> error, abort save.
2. Else clear those jobs' group and emit job-updated for each.

## UI

- Settings section `groups`
- Left list (order = chip order), right edit name; up/down/delete/add
- Job create/detail: select + free text that can create
- Filter chips: all, managed names in order, ungrouped, then orphan legacy labels if any

## Display

`resolveGroupLabel(groupValue, catalog)`:

1. match id -> name
2. match name -> name  
3. else raw string (orphan)
