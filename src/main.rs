use clap::{Parser, Subcommand};
use pulldown_cmark::{Options, Parser as MdParser};
use serde::Serialize;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use tinytemplate::format_unescaped;
use tinytemplate::TinyTemplate;

#[derive(Parser, Debug)]
#[command(version, about = "Bread: A minimal static site generator", long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand, Debug)]
enum Commands {
    Build {
        #[arg(short, long, default_value = "content")]
        content_dir: String,

        #[arg(short, long, default_value = "public")]
        output_dir: String,

        #[arg(short, long, default_value = "templates")]
        template_dir: String,
    },
}

#[derive(Serialize, Debug)]
struct PageContext {
    title: String,
    content: String,
    tags: String,
    keywords: String,
    date: String,
}

#[derive(Serialize, Debug)]
struct PostsContext {
    post_count: usize,
    posts: String,
    tag_options: String,
}

#[derive(Debug, Default)]
struct Frontmatter {
    title: Option<String>,
    date: Option<String>,
    tags: Option<Vec<String>>,
    slug: Option<String>,
}

impl Frontmatter {
    fn parse(content: &str) -> (Self, &str) {
        let mut frontmatter = Frontmatter::default();
        if !content.starts_with("---") {
            return (frontmatter, content);
        }
        let Some(end_pos) = content[3..].find("\n---") else {
            return (frontmatter, content);
        };
        let fm_section = &content[3..3 + end_pos];
        let markdown_content = &content[3 + end_pos + 4..];

        let mut lines = fm_section.lines().peekable();
        let mut current_key: Option<&str> = None;
        let mut tag_list: Vec<String> = Vec::new();

        while let Some(line) = lines.next() {
            let trimmed = line.trim();

            if trimmed.is_empty() {
                continue;
            }

            if trimmed.starts_with('-') {
                if current_key == Some("tags") {
                    let tag = trimmed[1..].trim().to_string();
                    if !tag.is_empty() {
                        tag_list.push(tag);
                    }
                }
                continue;
            }

            if let Some(colon_pos) = trimmed.find(':') {
                if current_key == Some("tags") && !tag_list.is_empty() {
                    frontmatter.tags = Some(tag_list.clone());
                    tag_list.clear();
                }

                let key = trimmed[..colon_pos].trim();
                let value = trimmed[colon_pos + 1..].trim();

                current_key = Some(key);

                match key {
                    "title" => frontmatter.title = Some(value.to_string()),
                    "date" => frontmatter.date = Some(value.to_string()),
                    "slug" => frontmatter.slug = Some(value.to_string()),
                    "tags" => {
                        if !value.is_empty() {
                            frontmatter.tags = Some(
                                value
                                    .split(',')
                                    .map(|s| s.trim().to_string())
                                    .filter(|s| !s.is_empty())
                                    .collect(),
                            );
                            current_key = None;
                        }
                    }
                    _ => {
                        current_key = None;
                    }
                }
            }
        }

        // Don't forget to save tags if we were collecting them at the end
        if current_key == Some("tags") && !tag_list.is_empty() {
            frontmatter.tags = Some(tag_list);
        }

        (frontmatter, markdown_content.trim_start())
    }
}

#[derive(Debug, Clone)]
struct PostMetadata {
    title: String,
    date: String,
    tags: Vec<String>,
    url: String,
}

fn process_markdown_file(
    input_path: &Path,
    output_dir: &Path,
    content_dir: &Path,
    tt: &TinyTemplate,
) -> io::Result<()> {
    let content = fs::read_to_string(input_path)?;
    let (frontmatter, markdown_content) = Frontmatter::parse(&content);
    let html_content = markdown_to_html(markdown_content);

    let output_filename = frontmatter
        .slug
        .as_ref()
        .map(|s| format!("{}.html", s))
        .or_else(|| {
            input_path
                .file_stem()
                .and_then(|s| s.to_str())
                .map(|s| format!("{}.html", s))
        })
        .unwrap_or_else(|| "output.html".to_string());

    let relative_path = input_path
        .parent()
        .and_then(|p| p.strip_prefix(content_dir).ok())
        .unwrap_or(Path::new(""));

    let output_subdir = output_dir.join(relative_path);
    if !output_subdir.exists() {
        fs::create_dir_all(&output_subdir)?;
    }

    let output_path = output_subdir.join(&output_filename);

    let title = frontmatter.title.unwrap_or_else(|| "Untitled".to_string());
    let date = frontmatter.date.unwrap_or_default();
    let tags = frontmatter.tags.unwrap_or_default();

    let tags_html = tags
        .iter()
        .map(|tag| {
            format!(
                "<span class=\"tag\">#{}</span>",
                tag.trim().replace(' ', "")
            )
        })
        .collect::<Vec<_>>()
        .join("");

    let context = PageContext {
        title,
        content: html_content,
        tags: tags_html,
        keywords: tags.join(", "),
        date,
    };

    let rendered = tt
        .render("base", &context)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    fs::write(&output_path, rendered)?;
    println!("  ✓ {} -> {}", input_path.display(), output_path.display());

    Ok(())
}

fn collect_post_metadata(md_file: &Path, content_path: &Path) -> io::Result<Option<PostMetadata>> {
    let content = fs::read_to_string(md_file)?;
    let (frontmatter, _) = Frontmatter::parse(&content);

    let output_filename = frontmatter
        .slug
        .as_ref()
        .map(|s| format!("{}.html", s))
        .or_else(|| {
            md_file
                .file_stem()
                .and_then(|s| s.to_str())
                .map(|s| format!("{}.html", s))
        })
        .unwrap_or_else(|| "output.html".to_string());

    // Skip index pages
    if output_filename.contains("index") {
        return Ok(None);
    }

    let relative_path = md_file
        .parent()
        .and_then(|p| p.strip_prefix(content_path).ok())
        .unwrap_or(Path::new(""));

    let url = if relative_path.as_os_str().is_empty() {
        format!("/{}", output_filename)
    } else {
        format!("/{}/{}", relative_path.display(), output_filename)
    };

    Ok(Some(PostMetadata {
        title: frontmatter.title.unwrap_or_else(|| "Untitled".to_string()),
        date: frontmatter.date.unwrap_or_default(),
        tags: frontmatter.tags.unwrap_or_default(),
        url,
    }))
}

fn generate_posts_page(
    posts: &[PostMetadata],
    output_dir: &Path,
    tt: &TinyTemplate,
) -> io::Result<()> {
    let post_html: String = posts
        .iter()
        .map(|post| {
            let tags_html = post
                .tags
                .iter()
                .map(|tag| {
                    let clean = tag.trim().replace(' ', "");
                    format!(
                        "<span class=\"tag clickable-tag\" data-tag=\"{}\">#{}</span>",
                        clean, clean
                    )
                })
                .collect::<Vec<_>>()
                .join("");

            format!(
                r#"          <div class="post-item">
            <h3><a href="/bread/{}">{}</a></h3>
            <div class="post-meta">
              <span class="post-date">{}</span>
              <span class="post-tags">{}</span>
            </div>
          </div>
"#,
                post.url, post.title, post.date, tags_html
            )
        })
        .collect();

    let mut all_tags: Vec<String> = posts
        .iter()
        .flat_map(|p| p.tags.iter().map(|t| t.trim().replace(' ', "")))
        .collect();
    all_tags.sort();
    all_tags.dedup();

    let tag_options: String = all_tags
        .iter()
        .map(|tag| format!(r#"        <option value="{}">#{}</option>"#, tag, tag))
        .collect();

    let posts_context = PostsContext {
        post_count: posts.len(),
        posts: post_html,
        tag_options,
    };

    let rendered = tt
        .render("posts", &posts_context)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    fs::write(output_dir.join("posts.html"), rendered)?;
    println!("  📝 Generated posts.html");

    Ok(())
}

fn markdown_to_html(markdown: &str) -> String {
    let mut options = Options::empty();
    options.insert(Options::ENABLE_STRIKETHROUGH);
    options.insert(Options::ENABLE_TABLES);
    options.insert(Options::ENABLE_FOOTNOTES);
    options.insert(Options::ENABLE_TASKLISTS);

    let parser = MdParser::new_ext(markdown, options);
    let mut html_output = String::new();
    pulldown_cmark::html::push_html(&mut html_output, parser);
    html_output
}

fn find_markdown_files(dir: &Path) -> io::Result<Vec<PathBuf>> {
    let mut md_files = Vec::new();

    if !dir.is_dir() {
        return Ok(md_files);
    }

    for entry in fs::read_dir(dir)? {
        let path = entry?.path();

        if path.is_dir() {
            md_files.extend(find_markdown_files(&path)?);
        } else if path.extension().and_then(|s| s.to_str()) == Some("md") {
            md_files.push(path);
        }
    }

    Ok(md_files)
}

fn copy_dir_recursive(source: &Path, destination: &Path) -> io::Result<()> {
    if !destination.exists() {
        fs::create_dir_all(destination)?;
    }

    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let source_path = entry.path();
        let dest_path = destination.join(entry.file_name());

        if source_path.is_dir() {
            copy_dir_recursive(&source_path, &dest_path)?;
        } else {
            fs::copy(&source_path, &dest_path)?;
            println!("  📎 Copied: {}", entry.file_name().to_string_lossy());
        }
    }

    Ok(())
}

fn build_site(content_dir: &str, output_dir: &str, template_dir: &str) -> io::Result<()> {
    println!("🔨 Building site...\n");

    let output_path = Path::new(output_dir);
    if !output_path.exists() {
        fs::create_dir_all(output_path)?;
        println!("  Created output directory: {}", output_dir);
    }

    // Load templates
    let template_dir_path = Path::new(template_dir);
    let base_template = fs::read_to_string(template_dir_path.join("base.html"))?;
    let posts_template = fs::read_to_string(template_dir_path.join("posts.html"))?;

    // Initialize template engine
    let mut tt = TinyTemplate::new();
    tt.set_default_formatter(&format_unescaped);
    tt.add_template("base", &base_template)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    tt.add_template("posts", &posts_template)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    // Find and process markdown files
    let content_path = Path::new(content_dir);
    let md_files = find_markdown_files(content_path)?;

    if md_files.is_empty() {
        println!("  ⚠ No markdown files found in {}", content_dir);
    } else {
        println!("  Found {} markdown file(s)\n", md_files.len());

        // Collect post metadata
        let mut posts: Vec<PostMetadata> = md_files
            .iter()
            .filter_map(|md_file| collect_post_metadata(md_file, content_path).ok().flatten())
            .collect();

        posts.sort_by(|a, b| b.date.cmp(&a.date));

        // Process all markdown files
        for md_file in &md_files {
            process_markdown_file(md_file, output_path, content_path, &tt)?;
        }

        // Generate posts page
        if !posts.is_empty() {
            generate_posts_page(&posts, output_path, &tt)?;
        }
    }

    // Copy static assets
    println!("\n📦 Copying static assets...\n");

    let static_path = Path::new("static");
    if static_path.exists() && static_path.is_dir() {
        for entry in fs::read_dir(static_path)? {
            let entry = entry?;
            let source_path = entry.path();
            let dest_path = output_path.join(entry.file_name());

            if source_path.is_dir() {
                copy_dir_recursive(&source_path, &dest_path)?;
            } else {
                fs::copy(&source_path, &dest_path)?;
                println!("  📎 Copied: {}", entry.file_name().to_string_lossy());
            }
        }
    } else {
        println!("  ℹ No static directory found. Create 'static/' for CSS/images.");
    }

    println!("\n✨ Site built successfully to {}/", output_dir);
    Ok(())
}

fn main() {
    let cli = Cli::parse();

    match cli.command {
        Commands::Build {
            content_dir,
            output_dir,
            template_dir,
        } => {
            if let Err(e) = build_site(&content_dir, &output_dir, &template_dir) {
                eprintln!("Error building site: {}", e);
                std::process::exit(1);
            }
        }
    }
}
