# daisyUI 5 - Base Reference

> Source: https://daisyui.com/llms.txt (daisyUI 5.5.x)

## Project-Specific Configuration

This project uses daisyUI 5 with the following setup:

**CSS file** (`desktop_app/tailwind.css`):
```css
@source "./src/**/*.{rs,html,css}";
@import "tailwindcss";
@import "./assets/theme.css";

@plugin "daisyui" {
    themes: emerald --default, dark;
}
```

**Custom theme** (`desktop_app/assets/theme.css`):
```css
@theme {
    --font-sans: "Inter", sans-serif;
}
```

- Default light theme: `emerald`
- Dark theme: `dark`
- Font: Inter (sans-serif)
- Installed via npm: `daisyui@5.5`, `tailwindcss@4.2`

---

## daisyUI 5 Install Notes

1. daisyUI 5 requires Tailwind CSS 4
2. `tailwind.config.js` is deprecated in Tailwind CSS v4. Do not use it. Tailwind CSS v4 only needs `@import "tailwindcss";` in the CSS file.
3. daisyUI 5 can be installed using `npm i -D daisyui@latest` and then adding `@plugin "daisyui";` to the CSS file

## daisyUI 5 Usage Rules

1. Style HTML elements by adding daisyUI class names: component class, part class names, and modifier class names
2. Components can be customized using Tailwind CSS utility classes if customization is not possible using existing daisyUI classes. For example `btn px-10` sets a custom horizontal padding
3. If Tailwind CSS utility classes don't work due to CSS specificity issues, use `!` at the end of the class. For example `btn bg-red-500!`. This is a last resort.
4. If a component doesn't exist in daisyUI, create it using Tailwind CSS utility classes
5. When using `flex` and `grid` for layout, make it responsive using Tailwind CSS responsive utility prefixes
6. Only allowed class names are existing daisyUI class names or Tailwind CSS utility classes
7. Ideally, no custom CSS is needed. Using daisyUI class names or Tailwind CSS utility classes is preferred
8. For placeholder images, use `https://picsum.photos/200/300` with desired size
9. Don't add a custom font unless necessary
10. Don't add `bg-base-100 text-base-content` to body unless necessary
11. For design decisions, use Refactoring UI book best practices

### Class Name Categories

These type names are for reference only and are not used in actual code:
- `component`: the required component class
- `part`: a child part of a component
- `style`: sets a specific style
- `behavior`: changes the behavior
- `color`: sets a specific color
- `size`: sets a specific size
- `placement`: sets a specific placement
- `direction`: sets a specific direction
- `modifier`: modifies the component or part
- `variant`: prefixes for utility classes that conditionally apply styles (syntax: `variant:utility-class`)

## daisyUI Config

daisyUI without config:
```css
@plugin "daisyui";
```

daisyUI with all default configs:
```css
@plugin "daisyui" {
  themes: light --default, dark --prefersdark;
  root: ":root";
  include: ;
  exclude: ;
  prefix: ;
  logs: true;
}
```

## daisyUI 5 Colors

### Color Names

- `primary`: Primary brand color
- `primary-content`: Foreground content color on primary
- `secondary`: Secondary brand color
- `secondary-content`: Foreground content color on secondary
- `accent`: Accent brand color
- `accent-content`: Foreground content color on accent
- `neutral`: Neutral dark color, for not-saturated UI parts
- `neutral-content`: Foreground content color on neutral
- `base-100`: Base surface color (blank backgrounds)
- `base-200`: Base color, darker shade (elevations)
- `base-300`: Base color, even darker shade (elevations)
- `base-content`: Foreground content color on base
- `info`: For informative/helpful messages
- `info-content`: Foreground content color on info
- `success`: For success/safe messages
- `success-content`: Foreground content color on success
- `warning`: For warning/caution messages
- `warning-content`: Foreground content color on warning
- `error`: For error/danger/destructive messages
- `error-content`: Foreground content color on error

### Color Rules

1. daisyUI adds semantic color names to Tailwind CSS colors
2. daisyUI color names can be used in utility classes like other Tailwind CSS color names (e.g., `bg-primary`)
3. daisyUI colors include CSS variables so they change based on the theme
4. There's no need to use `dark:` for daisyUI color names
5. **Only use daisyUI color names** so colors change automatically based on the theme
6. If a Tailwind CSS color name (like `red-500`) is used, it will be the same on all themes
7. If a daisyUI color name (like `primary`) is used, it will change based on the theme
8. Avoid Tailwind CSS color names for text colors because `text-gray-800` on `bg-base-100` would be unreadable on dark themes
9. `*-content` colors should have good contrast compared to their associated colors
10. When designing a page, use `base-*` colors for the majority. Use `primary` for important elements.

### Custom Theme with Custom Colors

A CSS file with a custom daisyUI theme:
```css
@import "tailwindcss";
@plugin "daisyui";
@plugin "daisyui/theme" {
  name: "mytheme";
  default: true;
  prefersdark: false;
  color-scheme: light;

  --color-base-100: oklch(98% 0.02 240);
  --color-base-200: oklch(95% 0.03 240);
  --color-base-300: oklch(92% 0.04 240);
  --color-base-content: oklch(20% 0.05 240);
  --color-primary: oklch(55% 0.3 240);
  --color-primary-content: oklch(98% 0.01 240);
  --color-secondary: oklch(70% 0.25 200);
  --color-secondary-content: oklch(98% 0.01 200);
  --color-accent: oklch(65% 0.25 160);
  --color-accent-content: oklch(98% 0.01 160);
  --color-neutral: oklch(50% 0.05 240);
  --color-neutral-content: oklch(98% 0.01 240);
  --color-info: oklch(70% 0.2 220);
  --color-info-content: oklch(98% 0.01 220);
  --color-success: oklch(65% 0.25 140);
  --color-success-content: oklch(98% 0.01 140);
  --color-warning: oklch(80% 0.25 80);
  --color-warning-content: oklch(20% 0.05 80);
  --color-error: oklch(65% 0.3 30);
  --color-error-content: oklch(98% 0.01 30);

  --radius-selector: 1rem;
  --radius-field: 0.25rem;
  --radius-box: 0.5rem;

  --size-selector: 0.25rem;
  --size-field: 0.25rem;

  --border: 1px;

  --depth: 1;
  --noise: 0;
}
```

#### Custom Theme Rules
- All CSS variables above are required
- Colors can be OKLCH or hex or other formats
- If generating a custom theme, do not include comments from the example above
