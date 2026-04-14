use anyhow::{Result};

/// Find the start and end line numbers for a section based on heading
fn find_section_bounds(document: &str, heading: &str) -> Result<(usize, usize)> {
    let lines: Vec<&str> = document.lines().collect();

    let example = if heading.starts_with('#') {
        heading.to_string()
    } else {
        format!("## {}", heading)
    };

    let start = lines.iter()
        .position(|line| line.trim() == heading)
        .ok_or_else(|| anyhow::anyhow!(
            "Heading '{}' not found. Headings must include the '#' prefix (e.g. '{}'). \
            Use list_file_headings to see available headings.",
            heading, example
        ))?;
    let level = heading.chars().take_while(|c| *c == '#').count();

    let end = lines[start + 1..]
        .iter()
        .position(|line| {
            let trimmed = line.trim();
            trimmed.starts_with('#') && trimmed.chars().take_while(|c| *c == '#').count() <= level
        });
    let end = match end {
        Some(pos) => pos + start + 1,
        None => lines.len(),
    };
    Ok((start, end))
}

/// Extracts the content under a markdown heading
pub fn extract_section(document: &str, heading: &str) -> Result<String> {
    let (start, end) = find_section_bounds(document, heading)?;
    let lines: Vec<&str> = document.lines().collect();
    let section = &lines[start + 1..end];
    Ok(section.join("\n").trim().to_string())
}

pub fn extract_section_headings(document: &str) -> Vec<String> {
    document.lines()
        .filter(|l| l.trim().starts_with("## "))
        .map(|l| l.trim().to_string())
        .collect()
}

/// Replaces the content under a markdown heading, preserving the heading itself and everything outside the section
pub fn replace_section(document: &str, heading: &str, new_content: &str) -> Result<String> {
    let (start, end) = find_section_bounds(document, heading)?;
    let lines: Vec<&str> = document.lines().collect();
    let start_section = &lines[..=start];
    let end_section = &lines[end..];

    let mut result: Vec<&str> = Vec::new();
    result.extend_from_slice(start_section);
    result.push("");
    result.push(new_content);
    result.push("");
    result.extend_from_slice(end_section);

    Ok(result.join("\n"))
}

pub fn section_sizes(document: &str) -> Vec<(String, usize)> {
    let headings = extract_section_headings(document);
    let mut section_sizes: Vec<(String, usize)> = Vec::new();
    let lines: Vec<&str> = document.lines().collect();
    for heading in headings {
        let Ok((start, end)) = find_section_bounds(document, &heading) else { continue };
        let section = &lines[start..end];
        let characters = section.join("\n").len();
        section_sizes.push((heading, characters));
    }
    section_sizes.push((String::from("__total__"), document.len()));
    section_sizes
}