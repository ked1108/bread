---
title: Welcome to My Static Site
date: 2025-10-04
tags: welcome, intro, rust
slug: index
---

# Welcome to Bread

Bread is minimal static site generator built with **Rust** that supports:

- Markdown with frontmatter
- Custom templates
- Nested directories
- Multiple pages

## Features

### Markdown Support

All standard CommonMark features are supported:

- **Bold** and *italic* text
- ~~Strikethrough~~
- `inline code` and code blocks
- Links and images
- Lists and tables

### Frontmatter

Each page can have metadata:

```
---
title: Page Title
date: 2025-10-04
tags: tag1, tag2
slug: custom-url
---
```


## Getting Started

Build your site with:

```
bread build
```

Your site will be generated in the `public/` directory

## All Posts
You can add a list of all your articles using the custom `post_list` command in your markdown

{{ post_list }}
