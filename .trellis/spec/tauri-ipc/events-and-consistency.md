# Events and Snapshot Consistency

> Consistency rules for `job-updated`, list refreshes, selected Job requests,
> artifact loads, and running-Job polling.

## Event Contract

The backend emits `job-updated` after persisting Job state. Its payload is the
complete latest `Job`, not a partial patch. The frontend listens with
`listen<Job>` and merges the snapshot into list/detail state.

Keep the event name and full-payload behavior stable unless the Rust emitter,
TypeScript listener, merge algorithm, and recovery strategy change together.

Persistence is more authoritative than notification. Event-delivery failure
must not cause an already persisted successful business transition to be
reclassified as failed.

## Multiple Update Channels

The UI receives Job state from:

- Explicit list refresh.
- Explicit selected-Job detail request.
- `job-updated` event.
- Three-second polling while the selected Job is running.
- Immediate command responses for operations that return a Job.

These are snapshots of one backend-owned state, not independent stores.

## Ordering Rules

`mergeJobListSnapshots` demonstrates current ordering:

- Compare `updated_at` and keep the newer Job snapshot.
- Sort the list by `created_at` descending.
- Preserve deletion tombstones so an older refresh cannot reinsert a deleted
  Job.
- Update selected detail only when the payload still matches the selected ID.

Any timestamp format change must preserve a reliable total ordering or replace
string comparison with an explicit version/sequence contract.

## Request-Generation Guards

The frontend uses separate generations for list refresh, detail/artifact load,
and log load. A response may update UI only if:

1. Its request generation is still current.
2. Its captured Job ID is still selected.
3. For logs, its captured log name is still active.
4. The Job has not been deleted/tombstoned.

Apply the same checks to errors. A rejected old request must not display an
error for the current Job.

## Optional Artifact Loading

After loading Job detail, logs, merged transcript, and summary are independent
optional reads. Load them with `Promise.allSettled` and update each only if the
request remains current. Clear old artifact text before or during a selection
change; a missing new artifact must not leave old content visible.

## Event Listener Lifecycle

React StrictMode can mount, clean up, and remount effects during development.
Listener effects must handle cleanup before the asynchronous `listen` call
resolves and must invoke the returned unlisten function exactly once.

Intervals for running-Job polling must be created only while needed and cleared
on status/selection changes and unmount.

## Delete and Workspace Switch

- Add a deletion tombstone before an in-flight refresh can complete.
- Remove the Job from list/detail state after backend deletion succeeds.
- Clear selected logs/transcript/summary when the selected Job disappears.
- When workspace changes, invalidate outstanding generations and replace all Job
  snapshots from the new workspace.
- Never merge a previous workspace's event/refresh into the new workspace.

## Avoid

- Applying every event/response unconditionally in arrival order.
- Assuming event delivery is guaranteed.
- Polling all Jobs continuously as a substitute for event handling.
- Keeping optional artifact text until the next successful read.
- Using one shared request counter for unrelated list/detail/log channels.
- Emitting a partial Job patch without a versioned patch-merge contract.

## Verification Scenarios

1. Switch Job A -> B while A detail and artifact requests are pending.
2. Switch logs rapidly while earlier log requests are pending.
3. Delete a Job while a list refresh is pending.
4. Receive an event newer than a refresh response and preserve the event.
5. Miss/delay an event and recover the selected running Job through polling.
6. Mount under StrictMode and confirm only one active listener/interval remains.
7. Switch workspace while old requests are pending and reject all old updates.
