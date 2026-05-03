use crate::error::{LynPdfError, Result};
use crate::types::DocumentMetadata;
use html5ever::parse_document;
use html5ever::tendril::TendrilSink;
use markup5ever_rcdom::{Handle, NodeData, RcDom};
use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum HtmlNodeKind {
    Document,
    Element(String),
    Text(String),
}

#[derive(Debug, Clone)]
pub struct HtmlNode {
    pub kind: HtmlNodeKind,
    pub attrs: HashMap<String, String>,
    pub children: Vec<HtmlNode>,
}

#[derive(Debug, Clone)]
pub struct HtmlLink {
    pub href: String,
    pub text: String,
}

impl HtmlNode {
    pub fn document(children: Vec<HtmlNode>) -> Self {
        Self {
            kind: HtmlNodeKind::Document,
            attrs: HashMap::new(),
            children,
        }
    }

    pub fn tag_name(&self) -> Option<&str> {
        match &self.kind {
            HtmlNodeKind::Element(tag) => Some(tag.as_str()),
            _ => None,
        }
    }

    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs.get(name).map(String::as_str)
    }

    pub fn classes(&self) -> impl Iterator<Item = &str> {
        self.attr("class")
            .unwrap_or("")
            .split_whitespace()
            .filter(|part| !part.is_empty())
    }
}

pub fn parse_html(input: &str) -> Result<HtmlNode> {
    let mut bytes = input.as_bytes();
    let dom = parse_document(RcDom::default(), Default::default())
        .from_utf8()
        .read_from(&mut bytes)
        .map_err(|err| LynPdfError::Html(err.to_string()))?;

    Ok(convert_handle(&dom.document))
}

pub fn extract_style_blocks(root: &HtmlNode) -> String {
    let mut css = String::new();
    collect_style_blocks(root, &mut css);
    css
}

pub fn find_body(root: &HtmlNode) -> &HtmlNode {
    find_first_tag(root, "body").unwrap_or(root)
}

pub fn extract_document_metadata(root: &HtmlNode) -> DocumentMetadata {
    let mut metadata = DocumentMetadata::default();

    if let Some(title_node) = find_first_tag(root, "title") {
        let title = collect_text(title_node);
        if !title.is_empty() {
            metadata.title = Some(title);
        }
    }

    collect_meta_tags(root, &mut metadata);

    if metadata.creator.is_none() {
        metadata.creator = Some(format!("LynPDF {}", crate::LYNPDF_RS_VERSION));
    }
    if metadata.producer.is_none() {
        metadata.producer = Some(format!("LynPDF RS {}", crate::LYNPDF_RS_VERSION));
    }

    metadata
}

pub fn collect_links(node: &HtmlNode) -> Vec<HtmlLink> {
    let mut links = Vec::new();
    collect_links_inner(node, &mut links);
    links
}

pub fn is_block_tag(tag: &str) -> bool {
    matches!(
        tag,
        "html"
            | "body"
            | "main"
            | "section"
            | "article"
            | "header"
            | "footer"
            | "div"
            | "p"
            | "h1"
            | "h2"
            | "h3"
            | "h4"
            | "h5"
            | "h6"
            | "ul"
            | "ol"
            | "li"
            | "table"
            | "thead"
            | "tbody"
            | "tr"
            | "td"
            | "th"
            | "pre"
            | "blockquote"
    )
}

pub fn is_text_container(tag: &str) -> bool {
    matches!(
        tag,
        "p" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "li" | "td" | "th" | "pre"
    )
}

pub fn collect_text(node: &HtmlNode) -> String {
    let mut output = String::new();
    collect_text_inner(node, &mut output);
    collapse_document_text(&output)
}

pub fn collect_text_preserving_whitespace(node: &HtmlNode) -> String {
    let mut output = String::new();
    collect_text_inner(node, &mut output);
    output
}

fn convert_handle(handle: &Handle) -> HtmlNode {
    let children = handle
        .children
        .borrow()
        .iter()
        .map(convert_handle)
        .collect::<Vec<_>>();

    match &handle.data {
        NodeData::Document => HtmlNode::document(children),
        NodeData::Text { contents } => HtmlNode {
            kind: HtmlNodeKind::Text(contents.borrow().to_string()),
            attrs: HashMap::new(),
            children,
        },
        NodeData::Element { name, attrs, .. } => {
            let attrs = attrs
                .borrow()
                .iter()
                .map(|attr| {
                    (
                        attr.name.local.to_string().to_ascii_lowercase(),
                        attr.value.to_string(),
                    )
                })
                .collect::<HashMap<_, _>>();
            HtmlNode {
                kind: HtmlNodeKind::Element(name.local.to_string().to_ascii_lowercase()),
                attrs,
                children,
            }
        }
        _ => HtmlNode::document(children),
    }
}

fn collect_style_blocks(node: &HtmlNode, css: &mut String) {
    if node.tag_name() == Some("style") {
        let mut text = String::new();
        collect_raw_text(node, &mut text);
        if !text.trim().is_empty() {
            css.push_str(&text);
            css.push('\n');
        }
    }

    for child in &node.children {
        collect_style_blocks(child, css);
    }
}

fn collect_raw_text(node: &HtmlNode, output: &mut String) {
    match &node.kind {
        HtmlNodeKind::Text(text) => output.push_str(text),
        _ => {
            for child in &node.children {
                collect_raw_text(child, output);
            }
        }
    }
}

fn find_first_tag<'a>(node: &'a HtmlNode, tag: &str) -> Option<&'a HtmlNode> {
    if node.tag_name() == Some(tag) {
        return Some(node);
    }
    for child in &node.children {
        if let Some(found) = find_first_tag(child, tag) {
            return Some(found);
        }
    }
    None
}

fn collect_meta_tags(node: &HtmlNode, metadata: &mut DocumentMetadata) {
    if node.tag_name() == Some("meta") {
        let name = node
            .attr("name")
            .map(|value| value.trim().to_ascii_lowercase())
            .unwrap_or_default();
        let content = node.attr("content").map(str::trim).unwrap_or("");
        if !content.is_empty() {
            match name.as_str() {
                "author" if metadata.author.is_none() => {
                    metadata.author = Some(content.to_string())
                }
                "subject" if metadata.subject.is_none() => {
                    metadata.subject = Some(content.to_string())
                }
                "description" if metadata.subject.is_none() => {
                    metadata.subject = Some(content.to_string())
                }
                "keywords" if metadata.keywords.is_none() => {
                    metadata.keywords = Some(content.to_string())
                }
                "creator" if metadata.creator.is_none() => {
                    metadata.creator = Some(content.to_string())
                }
                "producer" if metadata.producer.is_none() => {
                    metadata.producer = Some(content.to_string())
                }
                _ => {}
            }
        }
    }

    for child in &node.children {
        collect_meta_tags(child, metadata);
    }
}

fn collect_links_inner(node: &HtmlNode, links: &mut Vec<HtmlLink>) {
    if node.tag_name() == Some("a") {
        let href = node.attr("href").map(str::trim).unwrap_or("");
        if !href.is_empty() {
            let text = collect_text(node);
            if !text.is_empty() {
                links.push(HtmlLink {
                    href: href.to_string(),
                    text,
                });
            }
        }
    }

    for child in &node.children {
        collect_links_inner(child, links);
    }
}

fn collect_text_inner(node: &HtmlNode, output: &mut String) {
    match &node.kind {
        HtmlNodeKind::Text(text) => output.push_str(text),
        HtmlNodeKind::Element(tag) if tag == "br" => output.push('\n'),
        HtmlNodeKind::Element(tag) if tag == "script" || tag == "style" => {}
        _ => {
            for child in &node.children {
                collect_text_inner(child, output);
            }
        }
    }
}

fn collapse_document_text(input: &str) -> String {
    let mut output = String::new();
    let mut previous_space = false;

    for ch in input.chars() {
        if ch == '\n' {
            if !output.ends_with('\n') {
                output.push('\n');
            }
            previous_space = false;
        } else if ch.is_whitespace() {
            if !previous_space {
                output.push(' ');
                previous_space = true;
            }
        } else {
            output.push(ch);
            previous_space = false;
        }
    }

    output.trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_document_metadata_from_head() {
        let html = r#"
                        <html>
                            <head>
                                <title>Sample Title</title>
                                <meta name="author" content="Test Author">
                                <meta name="description" content="Sample Subject">
                                <meta name="keywords" content="a,b,c">
                            </head>
                            <body><p>Hello</p></body>
                        </html>
                "#;
        let root = parse_html(html).expect("html parses");
        let metadata = extract_document_metadata(&root);
        assert_eq!(metadata.title.as_deref(), Some("Sample Title"));
        assert_eq!(metadata.author.as_deref(), Some("Test Author"));
        assert_eq!(metadata.subject.as_deref(), Some("Sample Subject"));
        assert_eq!(metadata.keywords.as_deref(), Some("a,b,c"));
        let expected_creator = format!("LynPDF {}", crate::LYNPDF_RS_VERSION);
        let expected_producer = format!("LynPDF RS {}", crate::LYNPDF_RS_VERSION);
        assert_eq!(metadata.creator.as_deref(), Some(expected_creator.as_str()));
        assert_eq!(
            metadata.producer.as_deref(),
            Some(expected_producer.as_str())
        );
    }

    #[test]
    fn collects_anchors_with_rendered_text() {
        let html = r##"
                        <html>
                            <body>
                                <p>Go to <a href="#section-a">Section A</a></p>
                                <p><a href="https://example.com">https://example.com</a></p>
                            </body>
                        </html>
                "##;
        let root = parse_html(html).expect("html parses");
        let body = find_body(&root);
        let links = collect_links(body);
        assert_eq!(links.len(), 2);
        assert_eq!(links[0].href, "#section-a");
        assert_eq!(links[0].text, "Section A");
        assert_eq!(links[1].href, "https://example.com");
    }

    #[test]
    fn extracts_style_blocks_without_polluting_document_text() {
        let html = r#"
                    <html>
                        <head><style>@page { size: A4 landscape; }</style></head>
                        <body><p>Hello</p></body>
                    </html>
                "#;
        let root = parse_html(html).expect("html parses");
        let css = extract_style_blocks(&root);
        assert!(css.contains("size: A4 landscape"));
        assert_eq!(collect_text(find_body(&root)), "Hello");
    }
}
