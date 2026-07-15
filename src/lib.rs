//! Library crate for toMarkdownMCP: every conversion, vault, RAG, and
//! analysis module, shared by the MCP server binary (`main.rs`) and the
//! desktop GUI crate (`gui/`).

pub mod blockquote_extractor;
pub mod browser;
pub mod code_language_detector;
pub mod comment_extractor;
pub mod converter;
pub mod definition_list_converter;
pub mod doc_intel;
pub mod document_converter;
pub mod embeddings;
pub mod error;
pub mod feed_email_converter;
pub mod file_type;
pub mod form_extractor;
pub mod heading_analyzer;
pub mod html_converter;
pub mod image_extractor;
pub mod knowledge;
pub mod link_extractor;
pub mod llm;
pub mod markup_converter;
pub mod obsidian;
pub mod office_converter;
pub mod pipeline;
pub mod rag;
pub mod retrieval;
pub mod similarity;
pub mod sources;
pub mod table_converter;
pub mod textmetrics;
pub mod toc_generator;
pub mod tui;
pub mod webarchive_parser;
