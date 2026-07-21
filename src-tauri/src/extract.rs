//! Text extraction for binary document formats (PDF, DOCX). Plain text / markdown
//! are read directly on the frontend; these formats are decoded here from the
//! raw file bytes so their text can flow into the same RAG pipeline.

use crate::error::{AppError, AppResult};
use quick_xml::events::Event;
use quick_xml::Reader;
use std::io::Read;

/// Extract plain text from a document given its file name (used to pick the
/// format) and raw bytes.
pub fn extract_text(name: &str, bytes: &[u8]) -> AppResult<String> {
    let ext = name.rsplit_once('.').map(|(_, e)| e.to_lowercase());
    match ext.as_deref() {
        Some("pdf") => pdf_to_text(bytes),
        Some("docx") => docx_to_text(bytes),
        Some(other) => Err(AppError::Msg(format!(
            "formato não suportado para extração binária: .{other}"
        ))),
        None => Err(AppError::Msg("arquivo sem extensão".into())),
    }
}

fn pdf_to_text(bytes: &[u8]) -> AppResult<String> {
    pdf_extract::extract_text_from_mem(bytes)
        .map_err(|e| AppError::Msg(format!("falha ao ler PDF: {e}")))
}

/// A .docx is a ZIP; the body text lives in `word/document.xml` inside `<w:t>`
/// runs, with `<w:p>` marking paragraph boundaries.
fn docx_to_text(bytes: &[u8]) -> AppResult<String> {
    let cursor = std::io::Cursor::new(bytes);
    let mut zip = zip::ZipArchive::new(cursor)
        .map_err(|e| AppError::Msg(format!("DOCX inválido: {e}")))?;
    let mut xml = String::new();
    zip.by_name("word/document.xml")
        .map_err(|_| AppError::Msg("DOCX sem word/document.xml".into()))?
        .read_to_string(&mut xml)
        .map_err(|e| AppError::Msg(format!("falha ao ler DOCX: {e}")))?;

    let mut reader = Reader::from_str(&xml);
    reader.config_mut().trim_text(false);
    let mut out = String::new();
    let mut in_text = false;
    loop {
        match reader.read_event() {
            Ok(Event::Start(e)) if e.local_name().as_ref() == b"t" => in_text = true,
            Ok(Event::End(e)) if e.local_name().as_ref() == b"t" => in_text = false,
            Ok(Event::Text(e)) if in_text => {
                out.push_str(&e.unescape().unwrap_or_default());
            }
            // Paragraph and line breaks → newlines so chunks read naturally.
            Ok(Event::End(e)) if e.local_name().as_ref() == b"p" => out.push('\n'),
            Ok(Event::Empty(e)) if e.local_name().as_ref() == b"br" => out.push('\n'),
            Ok(Event::Eof) => break,
            Err(e) => return Err(AppError::Msg(format!("falha ao parsear DOCX: {e}"))),
            _ => {}
        }
    }
    Ok(out)
}
