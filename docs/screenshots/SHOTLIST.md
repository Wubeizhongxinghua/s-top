# Screenshot Shot List

This file is the capture checklist for README media.

## Required PNGs

1. `overview-hero.png`
   Show the first screen after launch.
   Include the header, tabs, overview table, trend panel, and footer hints.
   Prefer a moment where partition pressure and ownership differ clearly across rows.

2. `my-jobs.png`
   Show the `My Jobs` page.
   Include live queue data, `Where / Why`, `resource footprint`, and a long `Name` visible through horizontal scrolling.
   Prefer a selection where at least one running job and one pending job are visible.

3. `all-jobs.png`
   Show the `All Jobs` page.
   Include multiple users, mine highlighting, sort arrows, and mixed running / pending states.
   Keep `Mine-only` off so the page reads as a cluster-wide queue view.

4. `users.png`
   Show the `Users` page.
   Include the top user summary table and the lower selected-user job list.
   Make sure resource footprint and top partitions are visible.

5. `partition-detail.png`
   Show one busy partition in `Partition Detail`.
   Include the partition summary, trend panel, node-state distribution, and the partition-local jobs table.

6. `node-detail.png`
   Show the `Node Detail` page.
   Include active filters and the filtered job list.
   Prefer a node with visible `user`, `state`, `where`, and `why` differences.

7. `job-detail.png`
   Show the structured job detail modal.
   Include the grouped sections:
   - Basic
   - Resources
   - Scheduling
   - Placement / Reason
   - Paths / Extra
   Keep the underlying jobs page visible enough to show this is a modal.

8. `cancel-preview.png`
   Show the cancel confirmation modal.
   Include the preview table, allowed vs blocked items, and the confirmation buttons.
   Prefer a case where both cancellable and non-cancellable jobs appear together.

## Optional GIFs

1. `demo-overview.gif`
   Launch the app, move in Overview, change metric mode, and enter a partition.

2. `demo-search-and-sort.gif`
   In a jobs page:
   - start `/` search
   - filter incrementally
   - click a sortable header
   - use `Left` / `Right` to reveal wide columns

3. `demo-job-detail-and-cancel.gif`
   Open job detail, click outside to close, then open cancel preview.

4. `demo-node-filtering.gif`
   Show interactive `user` / `state` / `where` / `why` filtering in `Node Detail`.

## Capture Tips

- Use a wide terminal, ideally around `160x45` or larger.
- Prefer real queue data with visible differences rather than empty or all-zero screens.
- Keep the terminal chrome minimal so the TUI remains the visual focus.
- Capture both structure and state:
  - selection highlight
  - sort direction
  - filters
  - trend history
  - wide columns after horizontal movement
- For README coverage, the most important four images are:
  1. `overview-hero.png`
  2. `my-jobs.png`
  3. `users.png`
  4. `job-detail.png`
