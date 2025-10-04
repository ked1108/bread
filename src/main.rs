use clap::{Parser, Subcommand};
use pulldown_cmark::{Options, Parser as MdParser};
use serde::Serialize;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use tinytemplate::TinyTemplate;
use tinytemplate::format_unescaped;

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


/// Represents parsed frontmatter from a Markdown file
#[derive(Debug, Default)]
struct Frontmatter {
    title: Option<String>,
    date: Option<String>,
    tags: Option<Vec<String>>,
    slug: Option<String>,
}

impl Frontmatter {
    /// Parse frontmatter from the beginning of a Markdown file
    /// Expected format:
    /// ---
    /// key: value
    /// key: value
    /// ---
    fn parse(content: &str) -> (Self, &str) {
        let mut frontmatter = Frontmatter::default();
        
        if !content.starts_with("---") {
            return (frontmatter, content);
        }

        let rest = &content[3..];
        let end_marker = "\n---";
        
        if let Some(end_pos) = rest.find(end_marker) {
            let fm_section = &rest[..end_pos];
            let markdown_content = &rest[end_pos + 4..]; // Skip "\n---"

            for line in fm_section.lines() {
                let line = line.trim();
                if line.is_empty() {
                    continue;
                }

                if let Some(colon_pos) = line.find(':') {
                    let key = line[..colon_pos].trim();
                    let value = line[colon_pos + 1..].trim();

                    match key {
                        "title" => frontmatter.title = Some(value.to_string()),
                        "date" => frontmatter.date = Some(value.to_string()),
                        "slug" => frontmatter.slug = Some(value.to_string()),
                        "tags" => {
                            let tags: Vec<String> = value
                                .split(',')
                                .map(|s| s.trim().to_string())
                                .filter(|s| !s.is_empty())
                                .collect();
                            frontmatter.tags = Some(tags);
                        }
                        _ => {}
                    }
                }
            }

            return (frontmatter, markdown_content.trim_start());
        }

        (frontmatter, content)
    }
}

use std::collections::HashMap;

#[derive(Debug, Clone)]
struct PostMetadata {
    title: String,
    date: String,
    tags: Vec<String>,
    url: String,
}

fn generate_post_list(posts: &[PostMetadata]) -> String {
    if posts.is_empty() {
        return String::from("<p>No posts found.</p>");
    }

    let mut html = String::from("<div class=\"post-list\">\n");
    
    for post in posts {
        html.push_str(&format!(
            r#"  <article class="post-item">
    <h3><a href="{}">{}</a></h3>
    <div class="post-meta">
      <span class="post-date">{}</span>
      <span class="post-tags">{}</span>
    </div>
  </article>
"#,
            post.url,
            post.title,
            post.date,
            post.tags.iter()
                .map(|tag| format!("<span class=\"tag\">#{}</span>", tag.trim().replace(" ", "")))
                .collect::<Vec<String>>()
                .join("")
        ));
    }
    
    html.push_str("</div>\n");
    html
}
fn process_markdown_file_with_metadata(
    input_path: &Path,
    output_dir: &Path,
    content_dir: &Path,
    tt: &TinyTemplate,
    post_list_html: &str,
) -> io::Result<PostMetadata> {
    // Read the markdown file
    let content = fs::read_to_string(input_path)?;

    // Parse frontmatter and separate markdown content
    let (frontmatter, markdown_content) = Frontmatter::parse(&content);

    // Check if content contains {{ post_list }} placeholder
    let markdown_with_posts = markdown_content.replace("{{ post_list }}", post_list_html);

    // Convert markdown to HTML
    let html_content = markdown_to_html(&markdown_with_posts);

    // Determine output filename
    let output_filename = if let Some(slug) = &frontmatter.slug {
        format!("{}.html", slug)
    } else {
        input_path
            .file_stem()
            .and_then(|s| s.to_str())
            .map(|s| format!("{}.html", s))
            .unwrap_or_else(|| "output.html".to_string())
    };

    // Preserve directory structure
    let relative_path = input_path
        .parent()
        .and_then(|p| p.strip_prefix(content_dir).ok())
        .unwrap_or_else(|| Path::new(""));

    let output_subdir = output_dir.join(relative_path);
    
    if !output_subdir.exists() {
        fs::create_dir_all(&output_subdir)?;
    }

    let output_path = output_subdir.join(&output_filename);

    // Build relative URL for linking
    let url = if relative_path.as_os_str().is_empty() {
        format!("/{}", output_filename)
    } else {
        format!("/{}/{}", relative_path.display(), output_filename)
    };

    // Build context for template
    let title = frontmatter.title.clone().unwrap_or_else(|| "Untitled".to_string());
    let date = frontmatter.date.clone().unwrap_or_default();
    let tags = frontmatter.tags.clone().unwrap_or_default();

    // Create two versions of tags:
    // 1. Plain text for meta keywords
    // 2. HTML with styling for display
    let tags_plain = tags.join(", ");
    let tags_html = tags.iter()
        .map(|tag| format!("<span class=\"tag\">#{}</span>", tag.trim().replace(" ", "")))
        .collect::<Vec<String>>()
        .join("");

    let context = PageContext {
        title: title.clone(),
        content: html_content,
        tags: tags_html,           // HTML version for footer display
        keywords: tags_plain,      // Plain text for meta keywords
        date: date.clone(),
    };


    // Render template
    let rendered = tt
        .render("base", &context)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;

    // Write output file
    fs::write(&output_path, rendered)?;

    println!("  ✓ {} -> {}", input_path.display(), output_path.display());

    Ok(PostMetadata {
        title,
        date,
        tags,
        url,
    })
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

    if dir.is_dir() {
        for entry in fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_dir() {
                md_files.extend(find_markdown_files(&path)?);
            } else if path.extension().and_then(|s| s.to_str()) == Some("md") {
                md_files.push(path);
            }
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
        let file_name = entry.file_name();
        let dest_path = destination.join(&file_name);

        if source_path.is_dir() {
            copy_dir_recursive(&source_path, &dest_path)?;
        } else {
            fs::copy(&source_path, &dest_path)?;
            println!("  📎 Copied: {}", file_name.to_string_lossy());
        }
    }

    Ok(())
}

fn build_site(content_dir: &str, output_dir: &str, template_dir: &str) -> io::Result<()> {
    println!("🔨 Building site...\n");

    // Ensure output directory exists
    let output_path = Path::new(output_dir);
    if !output_path.exists() {
        fs::create_dir_all(output_path)?;
        println!("  Created output directory: {}", output_dir);
    }

    // Load template
    let template_path = Path::new(template_dir).join("base.html");
    let template_content = fs::read_to_string(&template_path)
        .map_err(|e| {
            io::Error::new(
                io::ErrorKind::NotFound,
                format!("Failed to read template at {}: {}", template_path.display(), e),
            )
        })?;

    // Initialize template engine
    let mut tt = TinyTemplate::new();
    tt.set_default_formatter(&format_unescaped);
    
    tt.add_template("base", &template_content)
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;

    // Find all markdown files
    let content_path = Path::new(content_dir);
    let md_files = find_markdown_files(content_path)?;

    if md_files.is_empty() {
        println!("  ⚠ No markdown files found in {}", content_dir);
    } else {
        println!("  Found {} markdown file(s)\n", md_files.len());

        // PASS 1: Collect metadata from all posts
        let mut posts: Vec<PostMetadata> = Vec::new();
        
        for md_file in &md_files {
            let content = fs::read_to_string(md_file)?;
            let (frontmatter, _) = Frontmatter::parse(&content);
            
            // Determine output filename and URL
            let output_filename = if let Some(slug) = &frontmatter.slug {
                format!("{}.html", slug)
            } else {
                md_file
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .map(|s| format!("{}.html", s))
                    .unwrap_or_else(|| "output.html".to_string())
            };

            let relative_path = md_file
                .parent()
                .and_then(|p| p.strip_prefix(content_path).ok())
                .unwrap_or_else(|| Path::new(""));

            let url = if relative_path.as_os_str().is_empty() {
                format!("/{}", output_filename)
            } else {
                format!("/{}/{}", relative_path.display(), output_filename)
            };

            // Skip index pages from post list
            if !output_filename.contains("index") {
                posts.push(PostMetadata {
                    title: frontmatter.title.unwrap_or_else(|| "Untitled".to_string()),
                    date: frontmatter.date.unwrap_or_default(),
                    tags: frontmatter.tags.unwrap_or_default(),
                    url,
                });
            }
        }

        // Sort posts by date (newest first)
        posts.sort_by(|a, b| b.date.cmp(&a.date));

        // Generate post list HTML
        let post_list_html = generate_post_list(&posts);

        // PASS 2: Process each markdown file with the post list
        for md_file in md_files {
            process_markdown_file_with_metadata(
                &md_file,
                output_path,
                content_path,
                &tt,
                &post_list_html,
            )?;
        }
    }

    // Copy static assets
    println!("\n📦 Copying static assets...\n");
    
    let static_path = Path::new("static");
    if static_path.exists() && static_path.is_dir() {
        for entry in fs::read_dir(static_path)? {
            let entry = entry?;
            let source_path = entry.path();
            let file_name = entry.file_name();
            let dest_path = output_path.join(&file_name);

            if source_path.is_dir() {
                copy_dir_recursive(&source_path, &dest_path)?;
            } else {
                fs::copy(&source_path, &dest_path)?;
                println!("  📎 Copied: {}", file_name.to_string_lossy());
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
