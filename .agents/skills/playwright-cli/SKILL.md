---
name: playwright-cli
description: Automate browser interactions, test web pages and work with Playwright tests.
allowed-tools: Bash(bun:*)
---

# Browser Automation with `bun playwright cli`

Run these commands from the project directory that contains the local Playwright installation. In this repository, use `apps/web` as the working directory.

## Quick start

```bash
# open new browser
bun playwright cli open
# navigate to a page
bun playwright cli goto https://playwright.dev
# interact with the page using refs from the snapshot
bun playwright cli click e15
bun playwright cli type "page.click"
bun playwright cli press Enter
# take a screenshot (rarely used, as snapshot is more common)
bun playwright cli screenshot
# close the browser
bun playwright cli close
```

## Commands

### Core

```bash
bun playwright cli open
# open and navigate right away
bun playwright cli open https://example.com/
bun playwright cli goto https://playwright.dev
bun playwright cli type "search query"
bun playwright cli click e3
bun playwright cli dblclick e7
# --submit presses Enter after filling the element
bun playwright cli fill e5 "user@example.com"  --submit
bun playwright cli drag e2 e8
# drop files or data onto an element (from outside the page)
bun playwright cli drop e4 --path=./image.png
bun playwright cli drop e4 --data="text/plain=hello world"
bun playwright cli hover e4
bun playwright cli select e9 "option-value"
bun playwright cli upload ./document.pdf
bun playwright cli check e12
bun playwright cli uncheck e12
bun playwright cli snapshot
# search the snapshot for text or a regexp, returns matching nodes with surrounding context
bun playwright cli find "Sign in"
bun playwright cli find --regex "Sign (in|up)"
# wrap the regexp in slashes to add flags, e.g. /i for case-insensitive
bun playwright cli find --regex "/sign (in|up)/i"
bun playwright cli eval "document.title"
bun playwright cli eval "el => el.textContent" e5
# get element id, class, or any attribute not visible in the snapshot
bun playwright cli eval "el => el.id" e5
bun playwright cli eval "el => el.getAttribute('data-testid')" e5
bun playwright cli dialog-accept
bun playwright cli dialog-accept "confirmation text"
bun playwright cli dialog-dismiss
bun playwright cli resize 1920 1080
bun playwright cli close
```

### Navigation

```bash
bun playwright cli go-back
bun playwright cli go-forward
bun playwright cli reload
```

### Keyboard

```bash
bun playwright cli press Enter
bun playwright cli press ArrowDown
bun playwright cli keydown Shift
bun playwright cli keyup Shift
```

### Mouse

```bash
bun playwright cli mousemove 150 300
bun playwright cli mousedown
bun playwright cli mousedown right
bun playwright cli mouseup
bun playwright cli mouseup right
bun playwright cli mousewheel 0 100
```

### Save as

```bash
bun playwright cli screenshot
bun playwright cli screenshot e5
bun playwright cli screenshot --filename=page.png
bun playwright cli screenshot --hires
bun playwright cli pdf --filename=page.pdf
```

### Tabs

```bash
bun playwright cli tab-list
bun playwright cli tab-new
bun playwright cli tab-new https://example.com/page
bun playwright cli tab-close
bun playwright cli tab-close 2
bun playwright cli tab-select 0
```

### Storage

```bash
bun playwright cli state-save
bun playwright cli state-save auth.json
bun playwright cli state-load auth.json

# Cookies
bun playwright cli cookie-list
bun playwright cli cookie-list --domain=example.com
bun playwright cli cookie-get session_id
bun playwright cli cookie-set session_id abc123
bun playwright cli cookie-set session_id abc123 --domain=example.com --httpOnly --secure
bun playwright cli cookie-delete session_id
bun playwright cli cookie-clear

# LocalStorage
bun playwright cli localstorage-list
bun playwright cli localstorage-get theme
bun playwright cli localstorage-set theme dark
bun playwright cli localstorage-delete theme
bun playwright cli localstorage-clear

# SessionStorage
bun playwright cli sessionstorage-list
bun playwright cli sessionstorage-get step
bun playwright cli sessionstorage-set step 3
bun playwright cli sessionstorage-delete step
bun playwright cli sessionstorage-clear
```

### Network

```bash
bun playwright cli route "**/*.jpg" --status=404
bun playwright cli route "https://api.example.com/**" --body='{"mock": true}'
bun playwright cli route-list
bun playwright cli unroute "**/*.jpg"
bun playwright cli unroute
```

### DevTools

```bash
bun playwright cli console
bun playwright cli console warning
bun playwright cli requests
bun playwright cli request 5
bun playwright cli run-code "async page => await page.context().grantPermissions(['geolocation'])"
bun playwright cli run-code --filename=script.js
bun playwright cli tracing-start
bun playwright cli tracing-stop

# record user actions in the browser, print them as Playwright code on stop
bun playwright cli recording-start
bun playwright cli recording-stop

bun playwright cli video-start video.webm
bun playwright cli video-chapter "Chapter Title" --description="Details" --duration=2000
bun playwright cli video-stop

# annotate each subsequent action (click, type, ...) with a callout naming the action and highlighting the target
bun playwright cli video-show-actions --duration=600 --position=top-right
bun playwright cli video-hide-actions

# launch the dashboard for UI review / design feedback — user annotates the page, you receive the annotated screenshot, snapshot, and notes
bun playwright cli show --annotate

# generate a Playwright locator for an element from its ref or selector
bun playwright cli generate-locator e5 --raw

# show a persistent highlight overlay for an element, optionally with a custom style
bun playwright cli highlight e5
bun playwright cli highlight e5 --style="outline: 3px dashed red"
# hide a single element highlight, or all page highlights when no target is given
bun playwright cli highlight e5 --hide
bun playwright cli highlight --hide
```

## Raw output

The global `--raw` option strips page status, generated code, and snapshot sections from the output, returning only the result value. Use it to pipe command output into other tools. Commands that don't produce output return nothing.

```bash
bun playwright cli --raw eval "JSON.stringify(performance.timing)" | jq '.loadEventEnd - .navigationStart'
bun playwright cli --raw eval "JSON.stringify([...document.querySelectorAll('a')].map(a => a.href))" > links.json
bun playwright cli --raw snapshot > before.yml
bun playwright cli click e5
bun playwright cli --raw snapshot > after.yml
diff before.yml after.yml
TOKEN=$(bun playwright cli --raw cookie-get session_id)
bun playwright cli --raw localstorage-get theme
```

For structured output wrapping every reply as JSON, pass --json
```bash
bun playwright cli list --json
```

## Open parameters
```bash
# Use specific browser when creating session
bun playwright cli open --browser=chrome
bun playwright cli open --browser=firefox
bun playwright cli open --browser=webkit
bun playwright cli open --browser=msedge

# Emulate a generic mobile device (Pixel 10 for Chromium, iPhone 17 for WebKit).
# Prefer this when a mobile layout is acceptable: mobile pages are usually
# lighter, so snapshots are smaller and cheaper.
bun playwright cli open --mobile
bun playwright cli open --device="iPhone 15"

# Use persistent profile (by default profile is in-memory)
bun playwright cli open --persistent
# Use persistent profile with custom directory
bun playwright cli open --profile=/path/to/profile

# Connect to browser via Playwright Extension
bun playwright cli attach --extension=chrome

# Connect to a running Chrome or Edge by channel name
bun playwright cli attach --cdp=chrome
bun playwright cli attach --cdp=msedge

# Connect to a running browser via CDP endpoint
bun playwright cli attach --cdp=http://localhost:9222

# Start with config file
bun playwright cli open --config=my-config.json

# Close the browser
bun playwright cli close
# Detach from an attached browser (leaves the external browser running)
bun playwright cli -s=msedge detach
# Delete user data for the default session
bun playwright cli delete-data
```

## URLs with `&` on Windows

On Windows, `cmd.exe` and PowerShell treat `&` as a command separator, so URLs with multiple query parameters get truncated before `bun playwright cli` runs. Escape `&` with `^&` in `cmd.exe`, or use `--%` in PowerShell:

```batch
bun playwright cli goto "https://example.com/?a=1^&b=2"
```

```powershell
bun playwright cli --% goto "https://example.com/?a=1&b=2"
```

## Snapshots

After each command, bun playwright cli provides a snapshot of the current browser state.

```bash
> bun playwright cli goto https://example.com
### Page
- Page URL: https://example.com/
- Page Title: Example Domain
### Snapshot
[Snapshot](.playwright-cli/page-2026-02-14T19-22-42-679Z.yml)
```

You can also take a snapshot on demand using `bun playwright cli snapshot` command. All the options below can be combined as needed.

```bash
# default - save to a file with timestamp-based name
bun playwright cli snapshot

# save to file, use when snapshot is a part of the workflow result
bun playwright cli snapshot --filename=after-click.yaml

# snapshot an element instead of the whole page
bun playwright cli snapshot "#main"

# limit snapshot depth for efficiency, take a partial snapshot afterwards
bun playwright cli snapshot --depth=4
bun playwright cli snapshot e34

# include each element's bounding box as [box=x,y,width,height]
bun playwright cli snapshot --boxes

# search a large snapshot instead of capturing it all — returns matching nodes
# with 3 lines of context around each match (like grep -C)
bun playwright cli find "Add to cart"
bun playwright cli find --regex "\\$[0-9]+\\.[0-9]{2}"
```

## Targeting elements

By default, use refs from the snapshot to interact with page elements.

```bash
# get snapshot with refs
bun playwright cli snapshot

# interact using a ref
bun playwright cli click e15
```

You can also use css selectors or Playwright locators.

```bash
# css selector
bun playwright cli click "#main > button.submit"

# role locator
bun playwright cli click "getByRole('button', { name: 'Submit' })"

# test id
bun playwright cli click "getByTestId('submit-button')"
```

## Browser Sessions

```bash
# create new browser session named "mysession" with persistent profile
bun playwright cli -s=mysession open example.com --persistent
# same with manually specified profile directory (use when requested explicitly)
bun playwright cli -s=mysession open example.com --profile=/path/to/profile
bun playwright cli -s=mysession click e6
bun playwright cli -s=mysession close  # stop a named browser
bun playwright cli -s=mysession delete-data  # delete user data for persistent session

bun playwright cli list
# Close all browsers
bun playwright cli close-all
# Forcefully kill all browser processes
bun playwright cli kill-all
```

## Installation

Use the project-local Playwright CLI through Bun:

```bash
bun playwright --version
```

Run all browser CLI commands through the local Playwright installation:

```bash
bun playwright cli --help
```

## Example: Form submission

```bash
bun playwright cli open https://example.com/form
bun playwright cli snapshot

bun playwright cli fill e1 "user@example.com"
bun playwright cli fill e2 "password123"
bun playwright cli click e3
bun playwright cli snapshot
bun playwright cli close
```

## Example: Multi-tab workflow

```bash
bun playwright cli open https://example.com
bun playwright cli tab-new https://example.com/other
bun playwright cli tab-list
bun playwright cli tab-select 0
bun playwright cli snapshot
bun playwright cli close
```

## Example: Debugging with DevTools

```bash
bun playwright cli open https://example.com
bun playwright cli click e4
bun playwright cli fill e7 "test"
bun playwright cli console
bun playwright cli requests
bun playwright cli close
```

```bash
bun playwright cli open https://example.com
bun playwright cli tracing-start
bun playwright cli click e4
bun playwright cli fill e7 "test"
bun playwright cli tracing-stop
bun playwright cli close
```

## Example: Interactive session

Ask the user for UI review or design feedback. The user draws boxes on the live page and types comments; you receive the annotated screenshot, the snapshot of the marked region, and the user's notes. Use this whenever the user asks for "UI review", "design feedback", or to "ask the user what they think / want / mean":

```bash
bun playwright cli open https://example.com
bun playwright cli show --annotate
```

## Specific tasks

* **Running and Debugging Playwright tests** [references/playwright-tests.md](references/playwright-tests.md)
* **Request mocking** [references/request-mocking.md](references/request-mocking.md)
* **Running Playwright code** [references/running-code.md](references/running-code.md)
* **Browser session management** [references/session-management.md](references/session-management.md)
* **Storage state (cookies, localStorage)** [references/storage-state.md](references/storage-state.md)
* **Test generation (plan / generate / heal)** [references/test-generation.md](references/test-generation.md)
* **Tracing** [references/tracing.md](references/tracing.md)
* **Video recording** [references/video-recording.md](references/video-recording.md)
* **Inspecting element attributes** [references/element-attributes.md](references/element-attributes.md)
