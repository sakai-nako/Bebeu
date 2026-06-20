# daisyUI 5 Components Reference (Part 1: accordion - hero)

> Source: https://daisyui.com/llms.txt (daisyUI 5.5.x)

## accordion

Accordion shows/hides content, only one item open at a time.

[Docs](https://daisyui.com/components/accordion/)

### Class names
- component: `collapse`
- part: `collapse-title`, `collapse-content`
- modifier: `collapse-arrow`, `collapse-plus`, `collapse-open`, `collapse-close`

### Syntax
```html
<div class="collapse {MODIFIER}">
  <input type="radio" name="{name}" checked="{checked}" />
  <div class="collapse-title">{title}</div>
  <div class="collapse-content">{CONTENT}</div>
</div>
```

### Rules
- Uses radio inputs. All radio inputs with the same name work together
- Use different names for different accordion groups on the same page
- Replace `{checked}` with `checked="checked"` to open by default

---

## alert

Alert informs users about important events.

[Docs](https://daisyui.com/components/alert/)

### Class names
- component: `alert`
- style: `alert-outline`, `alert-dash`, `alert-soft`
- color: `alert-info`, `alert-success`, `alert-warning`, `alert-error`
- direction: `alert-vertical`, `alert-horizontal`

### Syntax
```html
<div role="alert" class="alert {MODIFIER}">{CONTENT}</div>
```

### Rules
- Add `sm:alert-horizontal` for responsive layouts

---

## avatar

Avatars show a thumbnail.

[Docs](https://daisyui.com/components/avatar/)

### Class names
- component: `avatar`, `avatar-group`
- modifier: `avatar-online`, `avatar-offline`, `avatar-placeholder`

### Syntax
```html
<div class="avatar {MODIFIER}">
  <div>
    <img src="{image-url}" />
  </div>
</div>
```

### Rules
- Use `avatar-group` for containing multiple avatars
- Set custom sizes using `w-*` and `h-*`
- Use mask classes: `mask-squircle`, `mask-hexagon`, `mask-triangle`

---

## badge

Badges inform the user of the status of specific data.

[Docs](https://daisyui.com/components/badge/)

### Class names
- component: `badge`
- style: `badge-outline`, `badge-dash`, `badge-soft`, `badge-ghost`
- color: `badge-neutral`, `badge-primary`, `badge-secondary`, `badge-accent`, `badge-info`, `badge-success`, `badge-warning`, `badge-error`
- size: `badge-xs`, `badge-sm`, `badge-md`, `badge-lg`, `badge-xl`

### Syntax
```html
<span class="badge {MODIFIER}">Badge</span>
```

### Rules
- Can be used inside text or buttons
- Remove text for an empty badge

---

## breadcrumbs

Breadcrumbs helps users navigate.

[Docs](https://daisyui.com/components/breadcrumbs/)

### Class names
- component: `breadcrumbs`

### Syntax
```html
<div class="breadcrumbs">
  <ul><li><a>Link</a></li></ul>
</div>
```

### Rules
- Can contain icons inside the links
- If you set `max-width` or list gets larger than container, it will scroll

---

## button

Buttons allow the user to take actions.

[Docs](https://daisyui.com/components/button/)

### Class names
- component: `btn`
- color: `btn-neutral`, `btn-primary`, `btn-secondary`, `btn-accent`, `btn-info`, `btn-success`, `btn-warning`, `btn-error`
- style: `btn-outline`, `btn-dash`, `btn-soft`, `btn-ghost`, `btn-link`
- behavior: `btn-active`, `btn-disabled`
- size: `btn-xs`, `btn-sm`, `btn-md`, `btn-lg`, `btn-xl`
- modifier: `btn-wide`, `btn-block`, `btn-square`, `btn-circle`

### Syntax
```html
<button class="btn {MODIFIER}">Button</button>
```

### Rules
- Can be used on `<button>`, `<a>`, `<input>` tags
- Can have an icon before or after the text
- Set `tabindex="-1" role="button" aria-disabled="true"` to disable with a class name

---

## calendar

Calendar includes styles for different calendar libraries.

[Docs](https://daisyui.com/components/calendar/)

### Class names
- component: `cally` (Cally web component), `pika-single` (Pikaday), `react-day-picker` (React Day Picker)

### Syntax
For Cally:
```html
<calendar-date class="cally">{CONTENT}</calendar-date>
```
For Pikaday:
```html
<input type="text" class="input pika-single">
```

### Rules
- daisyUI supports Cally, Pikaday, React Day Picker

---

## card

Cards group and display content.

[Docs](https://daisyui.com/components/card/)

### Class names
- component: `card`
- part: `card-title`, `card-body`, `card-actions`
- style: `card-border`, `card-dash`
- modifier: `card-side`, `image-full`
- size: `card-xs`, `card-sm`, `card-md`, `card-lg`, `card-xl`

### Syntax
```html
<div class="card {MODIFIER}">
  <figure><img src="{image-url}" alt="{alt-text}" /></figure>
  <div class="card-body">
    <h2 class="card-title">{title}</h2>
    <p>{CONTENT}</p>
    <div class="card-actions">{actions}</div>
  </div>
</div>
```

### Rules
- `<figure>` and `<div class="card-body">` are optional
- Use `sm:card-horizontal` for responsive layouts
- If image is placed after `card-body`, image appears at bottom

---

## carousel

Carousel shows images or content in a scrollable area.

[Docs](https://daisyui.com/components/carousel/)

### Class names
- component: `carousel`
- part: `carousel-item`
- modifier: `carousel-start`, `carousel-center`, `carousel-end`
- direction: `carousel-horizontal`, `carousel-vertical`

### Syntax
```html
<div class="carousel {MODIFIER}">
  <div class="carousel-item">{CONTENT}</div>
</div>
```

### Rules
- Add `w-full` to each carousel item for full-width carousel

---

## chat

Chat bubbles show one line of conversation with author image, name, time, etc.

[Docs](https://daisyui.com/components/chat/)

### Class names
- component: `chat`
- part: `chat-image`, `chat-header`, `chat-footer`, `chat-bubble`
- placement: `chat-start`, `chat-end`
- color: `chat-bubble-neutral`, `chat-bubble-primary`, `chat-bubble-secondary`, `chat-bubble-accent`, `chat-bubble-info`, `chat-bubble-success`, `chat-bubble-warning`, `chat-bubble-error`

### Syntax
```html
<div class="chat {PLACEMENT}">
  <div class="chat-image"></div>
  <div class="chat-header"></div>
  <div class="chat-bubble {COLOR}">Message text</div>
  <div class="chat-footer"></div>
</div>
```

### Rules
- `{PLACEMENT}` is required: `chat-start` or `chat-end`
- To add avatar, use `<div class="chat-image avatar">`

---

## checkbox

Checkboxes select or deselect a value.

[Docs](https://daisyui.com/components/checkbox/)

### Class names
- component: `checkbox`
- color: `checkbox-primary`, `checkbox-secondary`, `checkbox-accent`, `checkbox-neutral`, `checkbox-success`, `checkbox-warning`, `checkbox-info`, `checkbox-error`
- size: `checkbox-xs`, `checkbox-sm`, `checkbox-md`, `checkbox-lg`, `checkbox-xl`

### Syntax
```html
<input type="checkbox" class="checkbox {MODIFIER}" />
```

---

## collapse

Collapse shows/hides content.

[Docs](https://daisyui.com/components/collapse/)

### Class names
- component: `collapse`
- part: `collapse-title`, `collapse-content`
- modifier: `collapse-arrow`, `collapse-plus`, `collapse-open`, `collapse-close`

### Syntax
```html
<div tabindex="0" class="collapse {MODIFIER}">
  <div class="collapse-title">{title}</div>
  <div class="collapse-content">{CONTENT}</div>
</div>
```

### Rules
- Instead of `tabindex="0"`, you can use `<input type="checkbox">` as first child
- Can also be a details/summary tag

---

## countdown

Countdown gives a transition effect when changing a number (0-999).

[Docs](https://daisyui.com/components/countdown/)

### Class names
- component: `countdown`

### Syntax
```html
<span class="countdown">
  <span style="--value:{number};">number</span>
</span>
```

### Rules
- `--value` and text must be a number between 0 and 999
- Change the span text and `--value` using JS
- Add `aria-live="polite"` and `aria-label="{number}"` for screen readers

---

## diff

Diff shows a side-by-side comparison of two items.

[Docs](https://daisyui.com/components/diff/)

### Class names
- component: `diff`
- part: `diff-item-1`, `diff-item-2`, `diff-resizer`

### Syntax
```html
<figure class="diff">
  <div class="diff-item-1">{item1}</div>
  <div class="diff-item-2">{item2}</div>
  <div class="diff-resizer"></div>
</figure>
```

### Rules
- Add `aspect-16/9` or other aspect ratio classes to maintain aspect ratio

---

## divider

Divider separates content vertically or horizontally.

[Docs](https://daisyui.com/components/divider/)

### Class names
- component: `divider`
- color: `divider-neutral`, `divider-primary`, `divider-secondary`, `divider-accent`, `divider-success`, `divider-warning`, `divider-info`, `divider-error`
- direction: `divider-vertical`, `divider-horizontal`
- placement: `divider-start`, `divider-end`

### Syntax
```html
<div class="divider {MODIFIER}">{text}</div>
```

### Rules
- Omit text for a blank divider

---

## dock

Dock (Bottom navigation) provides navigation options, sticks to bottom of screen.

[Docs](https://daisyui.com/components/dock/)

### Class names
- component: `dock`
- part: `dock-label`
- modifier: `dock-active`
- size: `dock-xs`, `dock-sm`, `dock-md`, `dock-lg`, `dock-xl`

### Syntax
```html
<div class="dock {MODIFIER}">
  <button>
    <svg>{icon}</svg>
    <span class="dock-label">Text</span>
  </button>
</div>
```

### Rules
- Add `dock-active` to active button
- Add `<meta name="viewport" content="viewport-fit=cover">` for iOS responsiveness

---

## drawer

Drawer is a grid layout that can show/hide a sidebar.

[Docs](https://daisyui.com/components/drawer/)

### Class names
- component: `drawer`
- part: `drawer-toggle`, `drawer-content`, `drawer-side`, `drawer-overlay`
- placement: `drawer-end`
- modifier: `drawer-open`
- variant: `is-drawer-open:`, `is-drawer-close:`

### Syntax
```html
<div class="drawer {MODIFIER}">
  <input id="my-drawer" type="checkbox" class="drawer-toggle" />
  <div class="drawer-content">{CONTENT}</div>
  <div class="drawer-side">
    <label for="my-drawer" aria-label="close sidebar" class="drawer-overlay"></label>
    <ul class="menu bg-base-200 min-h-full w-80 p-4">
      <li><button>Sidebar Item 1</button></li>
    </ul>
  </div>
</div>
```

### Rules
- `id` is required for `drawer-toggle` input
- `lg:drawer-open` makes sidebar visible on larger screens
- Use `<label for="my-drawer">` to toggle open/close
- All page content must be inside `drawer-content`
- Use `is-drawer-open:` and `is-drawer-close:` variant prefixes for conditional styles

---

## dropdown

Dropdown opens a menu or element when button is clicked.

[Docs](https://daisyui.com/components/dropdown/)

### Class names
- component: `dropdown`
- part: `dropdown-content`
- placement: `dropdown-start`, `dropdown-center`, `dropdown-end`, `dropdown-top`, `dropdown-bottom`, `dropdown-left`, `dropdown-right`
- modifier: `dropdown-hover`, `dropdown-open`, `dropdown-close`

### Syntax (details/summary)
```html
<details class="dropdown">
  <summary>Button</summary>
  <ul class="dropdown-content">{CONTENT}</ul>
</details>
```

### Syntax (popover API)
```html
<button popovertarget="{id}" style="anchor-name:--{anchor}">{button}</button>
<ul class="dropdown-content" popover id="{id}" style="position-anchor:--{anchor}">{CONTENT}</ul>
```

### Syntax (CSS focus)
```html
<div class="dropdown">
  <div tabindex="0" role="button">Button</div>
  <ul tabindex="-1" class="dropdown-content">{CONTENT}</ul>
</div>
```

### Rules
- Replace `{id}` and `{anchor}` with unique names
- Content can be any HTML element (not just `<ul>`)

---

## fab

FAB (Floating Action Button) stays in the bottom corner of screen.

[Docs](https://daisyui.com/components/fab/)

### Class names
- component: `fab`
- part: `fab-close`, `fab-main-action`
- modifier: `fab-flower`

### Syntax (single FAB)
```html
<div class="fab">
  <button class="btn btn-lg btn-circle">{Icon}</button>
</div>
```

### Syntax (with speed dial)
```html
<div class="fab">
  <div tabindex="0" role="button" class="btn btn-lg btn-circle btn-primary">{IconOriginal}</div>
  <button class="btn btn-lg btn-circle">{Icon1}</button>
  <button class="btn btn-lg btn-circle">{Icon2}</button>
  <button class="btn btn-lg btn-circle">{Icon3}</button>
</div>
```

### Syntax (flower shape)
```html
<div class="fab fab-flower">
  <div tabindex="0" role="button" class="btn btn-lg btn-circle btn-primary">{IconOriginal}</div>
  <button class="fab-main-action btn btn-circle btn-lg">{IconMainAction}</button>
  <button class="btn btn-lg btn-circle">{Icon1}</button>
  <button class="btn btn-lg btn-circle">{Icon2}</button>
</div>
```

### Rules
- Use SVG icons for `{Icon*}`
- `{IconOriginal}` = icon before opening FAB
- `{IconMainAction}` = icon after opening FAB
- Use `fab-close` for a close button that replaces the original

---

## fieldset

Fieldset groups related form elements.

[Docs](https://daisyui.com/components/fieldset/)

### Class names
- component: `fieldset`, `label`
- part: `fieldset-legend`

### Syntax
```html
<fieldset class="fieldset">
  <legend class="fieldset-legend">{title}</legend>
  {CONTENT}
  <p class="label">{description}</p>
</fieldset>
```

---

## file-input

File Input is for uploading files.

[Docs](https://daisyui.com/components/file-input/)

### Class names
- component: `file-input`
- style: `file-input-ghost`
- color: `file-input-neutral`, `file-input-primary`, `file-input-secondary`, `file-input-accent`, `file-input-info`, `file-input-success`, `file-input-warning`, `file-input-error`
- size: `file-input-xs`, `file-input-sm`, `file-input-md`, `file-input-lg`, `file-input-xl`

### Syntax
```html
<input type="file" class="file-input {MODIFIER}" />
```

---

## filter

Filter is a group of radio buttons. Choosing one hides others and shows a reset button.

[Docs](https://daisyui.com/components/filter/)

### Class names
- component: `filter`
- part: `filter-reset`

### Syntax (HTML form)
```html
<form class="filter">
  <input class="btn btn-square" type="reset" value="x"/>
  <input class="btn" type="radio" name="{NAME}" aria-label="Tab 1"/>
  <input class="btn" type="radio" name="{NAME}" aria-label="Tab 2"/>
</form>
```

### Syntax (without form)
```html
<div class="filter">
  <input class="btn filter-reset" type="radio" name="{NAME}" aria-label="x"/>
  <input class="btn" type="radio" name="{NAME}" aria-label="Tab 1"/>
  <input class="btn" type="radio" name="{NAME}" aria-label="Tab 2"/>
</div>
```

### Rules
- Each set of radio inputs must have unique `name` attributes
- Use `<form>` when possible; use `<div>` only when a form is not feasible
- Use `filter-reset` class for the reset button

---

## footer

Footer contains logo, copyright notice, and links.

[Docs](https://daisyui.com/components/footer/)

### Class names
- component: `footer`
- part: `footer-title`
- placement: `footer-center`
- direction: `footer-horizontal`, `footer-vertical`

### Syntax
```html
<footer class="footer {MODIFIER}">
  <nav>
    <h6 class="footer-title">Title</h6>
    <a class="link link-hover">Link</a>
  </nav>
</footer>
```

### Rules
- Use `sm:footer-horizontal` for responsive layouts
- Suggestion: use `base-200` for background color

---

## hero

Hero displays a large box or image with title and description.

[Docs](https://daisyui.com/components/hero/)

### Class names
- component: `hero`
- part: `hero-content`, `hero-overlay`

### Syntax
```html
<div class="hero {MODIFIER}">
  <div class="hero-content">{CONTENT}</div>
</div>
```

### Rules
- Use `hero-content` for text content
- Use `hero-overlay` to overlay the background image with a color
- Content can contain a figure
