# daisyUI 5 Components Reference (Part 2: hover-3d - validator)

> Source: https://daisyui.com/llms.txt (daisyUI 5.5.x)

## hover-3d

Hover 3D adds a 3D hover effect to content. Tilts and rotates based on mouse position.

[Docs](https://daisyui.com/components/hover-3d/)

### Class names
- component: `hover-3d`

### Syntax
```html
<div class="hover-3d my-12 mx-2">
  <figure class="max-w-100 rounded-2xl">
    <img src="{image-url}" alt="3D card" />
  </figure>
  <div></div><div></div><div></div><div></div>
  <div></div><div></div><div></div><div></div>
</div>
```

### Rules
- Can be `<div>` or `<a>`
- Must have exactly 9 direct children: first is main content, other 8 are empty `<div>`s for hover zones
- Content inside should be non-interactive (no buttons, links, inputs)

---

## hover-gallery

Hover Gallery shows multiple images; hovering horizontally reveals others. Useful for product cards.

[Docs](https://daisyui.com/components/hover-gallery/)

### Class names
- component: `hover-gallery`

### Syntax
```html
<figure class="hover-gallery max-w-60">
  <img src="{image-1}" />
  <img src="{image-2}" />
  <img src="{image-3}" />
</figure>
```

### Rules
- Can be `<div>` or `<figure>`
- Up to 10 images
- Needs a `max-width`; otherwise fills container width
- Images must be same dimensions

---

## indicator

Indicators place an element on the corner of another element.

[Docs](https://daisyui.com/components/indicator/)

### Class names
- component: `indicator`
- part: `indicator-item`
- placement: `indicator-start`, `indicator-center`, `indicator-end`, `indicator-top`, `indicator-middle`, `indicator-bottom`

### Syntax
```html
<div class="indicator">
  <span class="indicator-item">{indicator content}</span>
  <div>{main content}</div>
</div>
```

### Rules
- Add indicator elements before main content
- Default placement: `indicator-end indicator-top`

---

## input

Text Input is a simple input field.

[Docs](https://daisyui.com/components/input/)

### Class names
- component: `input`
- style: `input-ghost`
- color: `input-neutral`, `input-primary`, `input-secondary`, `input-accent`, `input-info`, `input-success`, `input-warning`, `input-error`
- size: `input-xs`, `input-sm`, `input-md`, `input-lg`, `input-xl`

### Syntax
```html
<input type="{type}" placeholder="Type here" class="input {MODIFIER}" />
```

### Rules
- Can be used with any input field type (text, password, email, etc.)
- Use `input` class for the parent when you have more than one element inside input

---

## join

Join groups multiple items (buttons, inputs, etc.) with border radius on first/last.

[Docs](https://daisyui.com/components/join/)

### Class names
- component: `join`, `join-item`
- direction: `join-vertical`, `join-horizontal`

### Syntax
```html
<div class="join {MODIFIER}">{CONTENT}</div>
```

### Rules
- Any direct child gets joined
- Any element with `join-item` will be affected
- Use `lg:join-horizontal` for responsive layouts

---

## kbd

Kbd displays keyboard shortcuts.

[Docs](https://daisyui.com/components/kbd/)

### Class names
- component: `kbd`
- size: `kbd-xs`, `kbd-sm`, `kbd-md`, `kbd-lg`, `kbd-xl`

### Syntax
```html
<kbd class="kbd {MODIFIER}">K</kbd>
```

---

## label

Label provides a name or title for an input field.

[Docs](https://daisyui.com/components/label/)

### Class names
- component: `label`, `floating-label`

### Syntax (regular)
```html
<label class="input">
  <span class="label">{label text}</span>
  <input type="text" placeholder="Type here" />
</label>
```

### Syntax (floating)
```html
<label class="floating-label">
  <input type="text" placeholder="Type here" class="input" />
  <span>{label text}</span>
</label>
```

### Rules
- The `input` class is for the parent element containing the input field and label
- Use `floating-label` for a label that floats above input when focused

---

## link

Link adds underline style to links.

[Docs](https://daisyui.com/components/link/)

### Class names
- component: `link`
- style: `link-hover`
- color: `link-neutral`, `link-primary`, `link-secondary`, `link-accent`, `link-success`, `link-info`, `link-warning`, `link-error`

### Syntax
```html
<a class="link {MODIFIER}">Click me</a>
```

---

## list

List is a vertical layout to display information in rows.

[Docs](https://daisyui.com/components/list/)

### Class names
- component: `list`, `list-row`
- modifier: `list-col-wrap`, `list-col-grow`

### Syntax
```html
<ul class="list">
  <li class="list-row">{CONTENT}</li>
</ul>
```

### Rules
- By default, second child of `list-row` fills remaining space
- Use `list-col-grow` on another child to change which fills space
- Use `list-col-wrap` to force an item to wrap

---

## loading

Loading shows an animation for loading state.

[Docs](https://daisyui.com/components/loading/)

### Class names
- component: `loading`
- style: `loading-spinner`, `loading-dots`, `loading-ring`, `loading-ball`, `loading-bars`, `loading-infinity`
- size: `loading-xs`, `loading-sm`, `loading-md`, `loading-lg`, `loading-xl`

### Syntax
```html
<span class="loading {MODIFIER}"></span>
```

---

## mask

Mask crops content to common shapes.

[Docs](https://daisyui.com/components/mask/)

### Class names
- component: `mask`
- style: `mask-squircle`, `mask-heart`, `mask-hexagon`, `mask-hexagon-2`, `mask-decagon`, `mask-pentagon`, `mask-diamond`, `mask-square`, `mask-circle`, `mask-star`, `mask-star-2`, `mask-triangle`, `mask-triangle-2`, `mask-triangle-3`, `mask-triangle-4`
- modifier: `mask-half-1`, `mask-half-2`

### Syntax
```html
<img class="mask {MODIFIER}" src="{image-url}" />
```

### Rules
- Style modifier is required
- Set custom sizes with `w-*` and `h-*`

---

## menu

Menu displays a list of links vertically or horizontally.

[Docs](https://daisyui.com/components/menu/)

### Class names
- component: `menu`
- part: `menu-title`, `menu-dropdown`, `menu-dropdown-toggle`
- modifier: `menu-disabled`, `menu-active`, `menu-focus`, `menu-dropdown-show`
- size: `menu-xs`, `menu-sm`, `menu-md`, `menu-lg`, `menu-xl`
- direction: `menu-vertical`, `menu-horizontal`

### Syntax
```html
<ul class="menu {MODIFIER}">
  <li><button>Item</button></li>
</ul>
```

### Rules
- Use `lg:menu-horizontal` for responsive layouts
- Use `menu-title` for list item title
- Use `<details>` tag to make submenus collapsible

---

## mockup-browser

Browser mockup looks like a browser window.

[Docs](https://daisyui.com/components/mockup-browser/)

### Class names
- component: `mockup-browser`
- part: `mockup-browser-toolbar`

### Syntax
```html
<div class="mockup-browser">
  <div class="mockup-browser-toolbar">{toolbar}</div>
  <div>{CONTENT}</div>
</div>
```

---

## mockup-code

Code mockup shows code in a code editor style box.

[Docs](https://daisyui.com/components/mockup-code/)

### Class names
- component: `mockup-code`

### Syntax
```html
<div class="mockup-code">
  <pre data-prefix="$"><code>npm i daisyui</code></pre>
</div>
```

### Rules
- Use `<pre data-prefix="{prefix}">` to show a prefix before each line
- To highlight a line, add background/text color

---

## mockup-phone

Phone mockup shows an iPhone-style mockup.

[Docs](https://daisyui.com/components/mockup-phone/)

### Class names
- component: `mockup-phone`
- part: `mockup-phone-camera`, `mockup-phone-display`

### Syntax
```html
<div class="mockup-phone">
  <div class="mockup-phone-camera"></div>
  <div class="mockup-phone-display">{CONTENT}</div>
</div>
```

---

## mockup-window

Window mockup shows an OS window style box.

[Docs](https://daisyui.com/components/mockup-window/)

### Class names
- component: `mockup-window`

### Syntax
```html
<div class="mockup-window">
  <div>{CONTENT}</div>
</div>
```

---

## modal

Modal shows a dialog or box when a button is clicked.

[Docs](https://daisyui.com/components/modal/)

### Class names
- component: `modal`
- part: `modal-box`, `modal-action`, `modal-backdrop`, `modal-toggle`
- modifier: `modal-open`
- placement: `modal-top`, `modal-middle`, `modal-bottom`, `modal-start`, `modal-end`

### Syntax (HTML dialog)
```html
<button onclick="my_modal.showModal()">Open modal</button>
<dialog id="my_modal" class="modal">
  <div class="modal-box">{CONTENT}</div>
  <form method="dialog" class="modal-backdrop"><button>close</button></form>
</dialog>
```

### Syntax (checkbox)
```html
<label for="my-modal" class="btn">Open modal</label>
<input type="checkbox" id="my-modal" class="modal-toggle" />
<div class="modal">
  <div class="modal-box">{CONTENT}</div>
  <label class="modal-backdrop" for="my-modal">Close</label>
</div>
```

### Rules
- Use unique IDs for each modal
- For dialog modals, add `<form method="dialog">` for closing

---

## navbar

Navbar shows a navigation bar on top of the page.

[Docs](https://daisyui.com/components/navbar/)

### Class names
- component: `navbar`
- part: `navbar-start`, `navbar-center`, `navbar-end`

### Syntax
```html
<div class="navbar">
  <div class="navbar-start">{left}</div>
  <div class="navbar-center">{center}</div>
  <div class="navbar-end">{right}</div>
</div>
```

### Rules
- Suggestion: use `base-200` for background color

---

## pagination

Pagination is a group of buttons using join.

[Docs](https://daisyui.com/components/pagination/)

### Class names
- component: `join`
- part: `join-item`

### Syntax
```html
<div class="join">
  <button class="join-item btn">1</button>
  <button class="join-item btn btn-active">2</button>
  <button class="join-item btn">3</button>
</div>
```

---

## progress

Progress bar shows progress of a task.

[Docs](https://daisyui.com/components/progress/)

### Class names
- component: `progress`
- color: `progress-neutral`, `progress-primary`, `progress-secondary`, `progress-accent`, `progress-info`, `progress-success`, `progress-warning`, `progress-error`

### Syntax
```html
<progress class="progress {MODIFIER}" value="50" max="100"></progress>
```

### Rules
- Must specify `value` and `max` attributes

---

## radial-progress

Radial progress shows circular progress.

[Docs](https://daisyui.com/components/radial-progress/)

### Class names
- component: `radial-progress`

### Syntax
```html
<div class="radial-progress" style="--value:70;" aria-valuenow="70" role="progressbar">70%</div>
```

### Rules
- `--value` must be 0-100
- Add `aria-valuenow="{value}"` for screen readers
- Use `--size` for size (default 5rem) and `--thickness` for indicator thickness

---

## radio

Radio buttons allow selecting one option.

[Docs](https://daisyui.com/components/radio/)

### Class names
- component: `radio`
- color: `radio-neutral`, `radio-primary`, `radio-secondary`, `radio-accent`, `radio-success`, `radio-warning`, `radio-info`, `radio-error`
- size: `radio-xs`, `radio-sm`, `radio-md`, `radio-lg`, `radio-xl`

### Syntax
```html
<input type="radio" name="{name}" class="radio {MODIFIER}" />
```

### Rules
- Each set of radio inputs should have unique `name` attributes

---

## range

Range slider selects a value by sliding.

[Docs](https://daisyui.com/components/range/)

### Class names
- component: `range`
- color: `range-neutral`, `range-primary`, `range-secondary`, `range-accent`, `range-success`, `range-warning`, `range-info`, `range-error`
- size: `range-xs`, `range-sm`, `range-md`, `range-lg`, `range-xl`

### Syntax
```html
<input type="range" min="0" max="100" value="40" class="range {MODIFIER}" />
```

### Rules
- Must specify `min` and `max` attributes

---

## rating

Rating is radio buttons for rating.

[Docs](https://daisyui.com/components/rating/)

### Class names
- component: `rating`
- modifier: `rating-half`, `rating-hidden`
- size: `rating-xs`, `rating-sm`, `rating-md`, `rating-lg`, `rating-xl`

### Syntax
```html
<div class="rating {MODIFIER}">
  <input type="radio" name="rating-1" class="mask mask-star" />
</div>
```

### Rules
- Each set should have unique `name` attributes
- Add `rating-hidden` for first radio to allow clearing

---

## select

Select picks a value from a list of options.

[Docs](https://daisyui.com/components/select/)

### Class names
- component: `select`
- style: `select-ghost`
- color: `select-neutral`, `select-primary`, `select-secondary`, `select-accent`, `select-info`, `select-success`, `select-warning`, `select-error`
- size: `select-xs`, `select-sm`, `select-md`, `select-lg`, `select-xl`

### Syntax
```html
<select class="select {MODIFIER}">
  <option>Option</option>
</select>
```

---

## skeleton

Skeleton shows a loading state placeholder.

[Docs](https://daisyui.com/components/skeleton/)

### Class names
- component: `skeleton`
- modifier: `skeleton-text`

### Syntax
```html
<div class="skeleton"></div>
<div class="skeleton skeleton-text">Loading data...</div>
```

### Rules
- Add `h-*` and `w-*` utility classes for dimensions

---

## stack

Stack visually puts elements on top of each other.

[Docs](https://daisyui.com/components/stack/)

### Class names
- component: `stack`
- modifier: `stack-top`, `stack-bottom`, `stack-start`, `stack-end`

### Syntax
```html
<div class="stack {MODIFIER}">{CONTENT}</div>
```

### Rules
- Use `w-*` and `h-*` classes to make all items same size

---

## stat

Stat shows numbers and data in a block.

[Docs](https://daisyui.com/components/stat/)

### Class names
- component: `stats`
- part: `stat`, `stat-title`, `stat-value`, `stat-desc`, `stat-figure`, `stat-actions`
- direction: `stats-horizontal`, `stats-vertical`

### Syntax
```html
<div class="stats {MODIFIER}">
  <div class="stat">
    <div class="stat-title">Title</div>
    <div class="stat-value">100</div>
    <div class="stat-desc">Description</div>
  </div>
</div>
```

### Rules
- Horizontal by default; use `stats-vertical` for vertical

---

## status

Status is a small icon showing current status (online, offline, error, etc.).

[Docs](https://daisyui.com/components/status/)

### Class names
- component: `status`
- color: `status-neutral`, `status-primary`, `status-secondary`, `status-accent`, `status-info`, `status-success`, `status-warning`, `status-error`
- size: `status-xs`, `status-sm`, `status-md`, `status-lg`, `status-xl`

### Syntax
```html
<span class="status {MODIFIER}"></span>
```

---

## steps

Steps show a list of steps in a process.

[Docs](https://daisyui.com/components/steps/)

### Class names
- component: `steps`
- part: `step`, `step-icon`
- color: `step-neutral`, `step-primary`, `step-secondary`, `step-accent`, `step-info`, `step-success`, `step-warning`, `step-error`
- direction: `steps-vertical`, `steps-horizontal`

### Syntax
```html
<ul class="steps {MODIFIER}">
  <li class="step step-primary">Step 1</li>
  <li class="step">Step 2</li>
</ul>
```

### Rules
- Add `step-primary` (or other color) to make a step active
- Use `step-icon` for custom icons
- Use `data-content="{value}"` on `<li>` for custom content

---

## swap

Swap toggles visibility of two elements.

[Docs](https://daisyui.com/components/swap/)

### Class names
- component: `swap`
- part: `swap-on`, `swap-off`, `swap-indeterminate`
- modifier: `swap-active`
- style: `swap-rotate`, `swap-flip`

### Syntax (checkbox)
```html
<label class="swap {MODIFIER}">
  <input type="checkbox" />
  <div class="swap-on">{content when active}</div>
  <div class="swap-off">{content when inactive}</div>
</label>
```

### Syntax (class name)
```html
<div class="swap {MODIFIER}">
  <div class="swap-on">{content when active}</div>
  <div class="swap-off">{content when inactive}</div>
</div>
```

### Rules
- Use hidden checkbox or add/remove `swap-active` class via JS
- Use `swap-indeterminate` for indeterminate state

---

## tab

Tabs show a list of links in a tabbed format.

[Docs](https://daisyui.com/components/tab/)

### Class names
- component: `tabs`
- part: `tab`, `tab-content`
- style: `tabs-box`, `tabs-border`, `tabs-lift`
- modifier: `tab-active`, `tab-disabled`
- placement: `tabs-top`, `tabs-bottom`

### Syntax (buttons)
```html
<div role="tablist" class="tabs {MODIFIER}">
  <button role="tab" class="tab">Tab 1</button>
  <button role="tab" class="tab tab-active">Tab 2</button>
</div>
```

### Syntax (radio inputs with content)
```html
<div role="tablist" class="tabs tabs-box">
  <input type="radio" name="my_tabs" class="tab" aria-label="Tab 1" />
  <div class="tab-content">{content 1}</div>
  <input type="radio" name="my_tabs" class="tab" aria-label="Tab 2" checked="checked" />
  <div class="tab-content">{content 2}</div>
</div>
```

### Rules
- Radio inputs are needed for tab content to work with tab click

---

## table

Table shows data in a table format.

[Docs](https://daisyui.com/components/table/)

### Class names
- component: `table`
- modifier: `table-zebra`, `table-pin-rows`, `table-pin-cols`
- size: `table-xs`, `table-sm`, `table-md`, `table-lg`, `table-xl`

### Syntax
```html
<div class="overflow-x-auto">
  <table class="table {MODIFIER}">
    <thead>
      <tr><th>Name</th></tr>
    </thead>
    <tbody>
      <tr><td>Data</td></tr>
    </tbody>
  </table>
</div>
```

### Rules
- Wrap in `overflow-x-auto` for horizontal scrolling on small screens

---

## text-rotate

Text Rotate shows 2-6 lines of text with infinite loop animation.

[Docs](https://daisyui.com/components/text-rotate/)

### Class names
- component: `text-rotate`

### Syntax
```html
<span class="text-rotate">
  <span>
    <span>Word 1</span>
    <span>Word 2</span>
    <span>Word 3</span>
  </span>
</span>
```

### Rules
- Must have one span/div inside containing 2-6 spans/divs
- Default loop duration: 10000ms
- Custom duration: `duration-{value}` utility class (e.g., `duration-12000`)
- Animation pauses on hover

---

## textarea

Textarea allows multi-line text input.

[Docs](https://daisyui.com/components/textarea/)

### Class names
- component: `textarea`
- style: `textarea-ghost`
- color: `textarea-neutral`, `textarea-primary`, `textarea-secondary`, `textarea-accent`, `textarea-info`, `textarea-success`, `textarea-warning`, `textarea-error`
- size: `textarea-xs`, `textarea-sm`, `textarea-md`, `textarea-lg`, `textarea-xl`

### Syntax
```html
<textarea class="textarea {MODIFIER}" placeholder="Bio"></textarea>
```

---

## theme-controller

Theme controller changes the page theme based on a checked checkbox/radio input.

[Docs](https://daisyui.com/components/theme-controller/)

### Class names
- component: `theme-controller`

### Syntax
```html
<input type="checkbox" value="{theme-name}" class="theme-controller" />
```

### Rules
- The `value` attribute must be a valid daisyUI theme name

---

## timeline

Timeline shows events in chronological order.

[Docs](https://daisyui.com/components/timeline/)

### Class names
- component: `timeline`
- part: `timeline-start`, `timeline-middle`, `timeline-end`
- modifier: `timeline-snap-icon`, `timeline-box`, `timeline-compact`
- direction: `timeline-vertical`, `timeline-horizontal`

### Syntax
```html
<ul class="timeline {MODIFIER}">
  <li>
    <div class="timeline-start">{start}</div>
    <div class="timeline-middle">{icon}</div>
    <div class="timeline-end">{end}</div>
  </li>
</ul>
```

### Rules
- Vertical by default
- `timeline-snap-icon` snaps icon to start instead of middle
- `timeline-compact` forces all items on one side

---

## toast

Toast stacks elements on the corner of the page.

[Docs](https://daisyui.com/components/toast/)

### Class names
- component: `toast`
- placement: `toast-start`, `toast-center`, `toast-end`, `toast-top`, `toast-middle`, `toast-bottom`

### Syntax
```html
<div class="toast {MODIFIER}">{CONTENT}</div>
```

---

## toggle

Toggle is a checkbox styled as a switch.

[Docs](https://daisyui.com/components/toggle/)

### Class names
- component: `toggle`
- color: `toggle-primary`, `toggle-secondary`, `toggle-accent`, `toggle-neutral`, `toggle-success`, `toggle-warning`, `toggle-info`, `toggle-error`
- size: `toggle-xs`, `toggle-sm`, `toggle-md`, `toggle-lg`, `toggle-xl`

### Syntax
```html
<input type="checkbox" class="toggle {MODIFIER}" />
```

---

## tooltip

Tooltip shows text when hovering or focusing on an element.

[Docs](https://daisyui.com/components/tooltip/)

### Class names
- component: `tooltip`
- placement: `tooltip-top`, `tooltip-bottom`, `tooltip-left`, `tooltip-right`
- color: `tooltip-primary`, `tooltip-secondary`, `tooltip-accent`, `tooltip-info`, `tooltip-success`, `tooltip-warning`, `tooltip-error`
- modifier: `tooltip-open`

### Syntax
```html
<div class="tooltip {MODIFIER}" data-tip="tooltip text">
  <button>Hover me</button>
</div>
```

### Rules
- Use `data-tip` attribute for tooltip text

---

## validator

Validator changes form element color to error or success based on validation.

[Docs](https://daisyui.com/components/validator/)

### Class names
- component: `validator`
- part: `validator-hint`

### Syntax
```html
<input type="{type}" class="input validator" required />
<p class="validator-hint">Error message</p>
```

### Rules
- Use with `input`, `select`, `textarea`
