# Implement: Job custom groups

## Order

1. Rust model: `Job.group`, `JobListItem.group`, create requests, `UpdateJobGroupRequest`, `Job::new` default, `to_list_item`.
2. Command `update_job_group` + create path apply + register in `lib.rs`.
3. Unit test: default deserialize without group; list item carries group.
4. TypeScript types + api wrapper.
5. UI: filter chips, card pill, create field, detail editor, merge/search include group.
6. PRODUCT_SPEC history row update.
7. Validate: `pnpm typecheck`, `pnpm build`, `cargo +stable test`, clippy if feasible.

## Validation

- Create job with group → appears under chip.
- Clear group → ungrouped filter.
- Running job: group edit disabled / rejected.
- Old jobs without field: ungrouped.
