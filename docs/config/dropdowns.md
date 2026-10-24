---
title: dropdowns
outline: [2, 3]
---

# dropdowns

<div v-pre>

User-defined dropdown panel opened by any module's click action.

Panels are named by `id` and opened from any bar module button with
`dropdown:<id>` (for example `dropdown:menu`). Content is either declared
statically via `items`, or produced by a `command` — the command's output
replaces the static items whenever it produces content.

Full walkthrough with examples at <https://wayle.app/guide/custom-modules>.

## General

| Field | Type | Default | Description |
|---|---|---|---|
| `id` | string | required | Unique identifier for this dropdown. |
| `title` | unknown | `null` | Title shown in the dropdown header. |
| `icon` | unknown | `null` | Icon name shown at the start of the dropdown header. |
| `width` | unknown | `null` | Panel width in pixels (before scale). |
| `command` | unknown | `null` | Shell command producing the dropdown content. |
| `mode` | [`ExecutionMode`](/config/types#execution-mode) | `"poll"` | Execution mode for the command. |
| `interval-ms` | u64 | `5000` | Polling interval in milliseconds. |
| `items` | array | `[]` | Static list of panel items. |
| `close-on-click` | bool | `true` | Close the dropdown after an item's command runs. |

::: details More about `id`

Referenced from module click actions as `dropdown:<id>`. Must be
unique across all custom dropdown definitions.

#### Example

```toml
[[modules.dropdowns]]
id = "menu"

# Reference in a module:
# left-click = "dropdown:menu"
```

:::

::: details More about `title`

A command's JSON output can override this at runtime via its `title`
field. If neither is set, the header is hidden entirely.

:::

::: details More about `icon`

A command's JSON output can override this at runtime via its `icon`
field. Should be a symbolic icon name from the icon theme
(e.g., `"ld-settings-symbolic"`).

:::

::: details More about `width`

Uses built-in default when omitted.

:::

::: details More about `command`

The command runs via `sh -c` and should output to stdout. Stderr is
discarded. Commands have a 30-second timeout.

#### Output Parsing

- If output starts with `{`: parsed as JSON. Recognized fields are
  `title`, `icon`, `width`, and `items`; anything else is optional.
- Otherwise: each non-empty line becomes a plain item without an
  action.

#### Behavior by Mode

- **poll**: Executed every `interval-ms` milliseconds
- **watch**: Spawned once, each stdout line is one complete panel
  payload. Restarts are controlled by `restart-policy`.

While no command output exists yet, static `items` are shown.
An empty output clears the list (the empty state is shown).

:::

::: details More about `mode`

| Mode | Behavior |
|------|----------|
| `poll` | Run command every `interval-ms` (default) |
| `watch` | Spawn long-running process, update on each stdout line |

Use `poll` for commands that return the panel state and exit.
Use `watch` for commands that stream panel updates.

:::

::: details More about `interval-ms`

Only applies to `poll` mode. Ignored in `watch` mode.

Set to `0` for manual mode: no timer is started. In manual mode, the
command still runs once at startup.

:::

::: details More about `items`

Each item is one row in the panel. Items may run a `command` when
clicked. When `command` output is present it replaces this list; use
static items for dropdowns without a backing process.

#### Example

```toml
[[modules.dropdowns]]
id = "menu"
title = "Quick actions"

[[modules.dropdowns.items]]
label = "Terminal"
icon = "ld-terminal-symbolic"
command = "foot"
```

:::

::: details More about `close-on-click`

Set to `false` for commands that produce feedback while the panel
stays open (e.g., clipboard pickers that redraw their own state).

:::

## Restart

| Field | Type | Default | Description |
|---|---|---|---|
| `restart-policy` | [`RestartPolicy`](/config/types#restart-policy) | `"never"` | Restart policy for watch mode. |
| `restart-interval-ms` | [`RestartDelay`](/config/types#restart-delay) | `1000` | Base restart delay in milliseconds for watch mode. |

::: details More about `restart-policy`

Only applies to `watch` mode. Ignored in `poll` mode.

| Policy | Behavior |
|--------|----------|
| `never` | Do not restart after exit |
| `on-exit` | Restart after any exit |
| `on-failure` | Restart only after non-zero/signal exit |

:::

::: details More about `restart-interval-ms`

Only applies to `watch` mode. Ignored in `poll` mode.

Used when `restart-policy` is `on-exit` or `on-failure`.
Delay increases exponentially on rapid failures, capped at 30 seconds.

:::

## Default configuration

Required fields (must be set in your config): `id`.

```toml
[[modules.dropdowns]]
mode = "poll"
interval-ms = 5000
restart-policy = "never"
restart-interval-ms = 1000
items = []
close-on-click = true
```


</div>
